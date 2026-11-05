package main

import (
	"bytes"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"
	_ "time/tzdata"
	"unicode/utf8"
)

const (
	defaultPort             = "8080"
	defaultDataDir          = "/data/events"
	defaultContextDir       = "/data/context"
	defaultRetentionDays    = 30
	defaultMaxTextChars     = 2000
	defaultPreviewChars     = 200
	defaultMaxRequestBytes  = 256 * 1024
	defaultMaxSnapshotBytes = 4 * 1024 * 1024
	cleanupInterval         = time.Hour
	maxEventIDLength        = 200
)

type config struct {
	Token          string
	Port           string
	DataDir        string
	ContextDir     string
	RetentionDays  int
	MaxTextChars   int
	PreviewChars   int
	MaxRequestSize int64
	Location       *time.Location
	AIAPIKey       string
	AIBaseURL      string
	AIModel        string
}

type incomingEvent struct {
	EventID     string   `json:"event_id"`
	CapturedAt  string   `json:"captured_at"`
	ContentType string   `json:"content_type"`
	Content     string   `json:"content"`
	AITags      []string `json:"ai_tags,omitempty"`
	SourceApp   string   `json:"source_app,omitempty"`
}

type storedEvent struct {
	EventID       string   `json:"event_id"`
	CapturedAt    string   `json:"captured_at"`
	ReceivedAt    string   `json:"received_at"`
	ContentType   string   `json:"content_type"`
	Content       string   `json:"content"`
	AITags        []string `json:"ai_tags,omitempty"`
	SourceApp     string   `json:"source_app,omitempty"`
	OriginalChars int      `json:"original_chars"`
	ContentSHA256 string   `json:"content_sha256"`
	Truncated     bool     `json:"truncated"`
}

type contextSnapshotRequest struct {
	CapturedAt  string                  `json:"captured_at"`
	SourceApp   string                  `json:"source_app"`
	WindowTitle string                  `json:"window_title"`
	ImageFormat string                  `json:"image_format,omitempty"`
	ImageBase64 string                  `json:"image_base64,omitempty"`
	Engine      string                  `json:"engine,omitempty"`
	Messages    []storedDialogueMessage `json:"messages,omitempty"`
}

type storedDialogueMessage struct {
	Speaker string `json:"speaker"`
	Role    string `json:"role"`
	Time    string `json:"time,omitempty"`
	Content string `json:"content"`
}

type storedContextSnapshot struct {
	CapturedAt  string                  `json:"captured_at"`
	ReceivedAt  string                  `json:"received_at"`
	SourceApp   string                  `json:"source_app"`
	WindowTitle string                  `json:"window_title"`
	Model       string                  `json:"model,omitempty"`
	RawResponse string                  `json:"raw_response,omitempty"`
	Messages    []storedDialogueMessage `json:"messages,omitempty"`
	Error       string                  `json:"error,omitempty"`
}

type openAIMessageContent struct {
	Type     string          `json:"type"`
	Text     string          `json:"text,omitempty"`
	ImageURL *openAIImageURL `json:"image_url,omitempty"`
}

type openAIImageURL struct {
	URL string `json:"url"`
}

type openAIChatMessage struct {
	Role    string                 `json:"role"`
	Content []openAIMessageContent `json:"content"`
}

type openAIChatRequest struct {
	Model    string              `json:"model"`
	Messages []openAIChatMessage `json:"messages"`
}

type openAIChatResponse struct {
	Choices []struct {
		Message struct {
			Content string `json:"content"`
		} `json:"message"`
	} `json:"choices"`
	Error *struct {
		Message string `json:"message"`
	} `json:"error,omitempty"`
}

type collector struct {
	cfg       config
	mu        sync.Mutex
	rateMu    sync.Mutex
	rate      map[string][]time.Time
	visionSem chan struct{}
}

func main() {
	cfg, err := loadConfig()
	if err != nil {
		log.Fatal(err)
	}

	if err := os.MkdirAll(cfg.DataDir, 0o750); err != nil {
		log.Fatalf("create data directory: %v", err)
	}
	if err := os.MkdirAll(cfg.ContextDir, 0o750); err != nil {
		log.Fatalf("create context directory: %v", err)
	}

	c := &collector{
		cfg:       cfg,
		rate:      make(map[string][]time.Time),
		visionSem: make(chan struct{}, 2),
	}
	c.pruneOldDays(time.Now())
	go c.cleanupLoop()

	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", c.handleHealth)
	mux.HandleFunc("/v1/clipboard-events", c.handleEvent)
	mux.HandleFunc("/v1/context-snapshots", c.handleContextSnapshot)

	server := &http.Server{
		Addr:              ":" + cfg.Port,
		Handler:           loggingMiddleware(mux),
		ReadHeaderTimeout: 5 * time.Second,
		ReadTimeout:       30 * time.Second,
		WriteTimeout:      30 * time.Second,
		IdleTimeout:       60 * time.Second,
	}

	log.Printf("qollect collector listening on :%s, data=%s, context=%s, retention=%dd", cfg.Port, cfg.DataDir, cfg.ContextDir, cfg.RetentionDays)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		log.Fatal(err)
	}
}

func loadConfig() (config, error) {
	token := strings.TrimSpace(os.Getenv("QOLLECT_TOKEN"))
	if token == "" {
		return config{}, errors.New("QOLLECT_TOKEN is required")
	}

	retentionDays, err := envInt("QOLLECT_RETENTION_DAYS", defaultRetentionDays, 1, 3650)
	if err != nil {
		return config{}, err
	}
	maxTextChars, err := envInt("QOLLECT_MAX_TEXT_CHARS", defaultMaxTextChars, 200, 1000000)
	if err != nil {
		return config{}, err
	}
	previewChars, err := envInt("QOLLECT_PREVIEW_CHARS", defaultPreviewChars, 1, 10000)
	if err != nil {
		return config{}, err
	}

	locationName := strings.TrimSpace(os.Getenv("QOLLECT_TIMEZONE"))
	if locationName == "" {
		locationName = "UTC"
	}
	location, err := time.LoadLocation(locationName)
	if err != nil {
		return config{}, fmt.Errorf("load QOLLECT_TIMEZONE %q: %w", locationName, err)
	}

	aiAPIKey := strings.TrimSpace(os.Getenv("QOLLECT_AI_API_KEY"))
	aiBaseURL := envString("QOLLECT_AI_BASE_URL", "https://api.siliconflow.cn/v1")
	aiModel := envString("QOLLECT_AI_MODEL", "Qwen/Qwen3-VL-32B-Instruct")

	return config{
		Token:          token,
		Port:           envString("QOLLECT_PORT", defaultPort),
		DataDir:        envString("QOLLECT_DATA_DIR", defaultDataDir),
		ContextDir:     envString("QOLLECT_CONTEXT_DIR", defaultContextDir),
		RetentionDays:  retentionDays,
		MaxTextChars:   maxTextChars,
		PreviewChars:   previewChars,
		MaxRequestSize: defaultMaxRequestBytes,
		Location:       location,
		AIAPIKey:       aiAPIKey,
		AIBaseURL:      aiBaseURL,
		AIModel:        aiModel,
	}, nil
}

func envString(name, fallback string) string {
	if value := strings.TrimSpace(os.Getenv(name)); value != "" {
		return value
	}
	return fallback
}

func envInt(name string, fallback, min, max int) (int, error) {
	value := strings.TrimSpace(os.Getenv(name))
	if value == "" {
		return fallback, nil
	}
	parsed, err := strconv.Atoi(value)
	if err != nil || parsed < min || parsed > max {
		return 0, fmt.Errorf("%s must be an integer between %d and %d", name, min, max)
	}
	return parsed, nil
}

func (c *collector) handleHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		w.WriteHeader(http.StatusMethodNotAllowed)
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (c *collector) handleEvent(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		w.WriteHeader(http.StatusMethodNotAllowed)
		return
	}
	if !c.authorized(r) {
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
		return
	}
	if !c.allowRequest(clientIP(r)) {
		writeJSON(w, http.StatusTooManyRequests, map[string]string{"error": "rate limit exceeded"})
		return
	}

	r.Body = http.MaxBytesReader(w, r.Body, c.cfg.MaxRequestSize)
	defer r.Body.Close()

	var event incomingEvent
	decoder := json.NewDecoder(r.Body)
	if err := decoder.Decode(&event); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid JSON"})
		return
	}

	stored, err := c.normalize(event)
	if err != nil {
		status := http.StatusBadRequest
		if strings.Contains(err.Error(), "content type") {
			status = http.StatusUnsupportedMediaType
		}
		writeJSON(w, status, map[string]string{"error": err.Error()})
		return
	}

	if err := c.append(stored); err != nil {
		log.Printf("append event failed: %v", err)
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "store failed"})
		return
	}

	writeJSON(w, http.StatusAccepted, map[string]interface{}{
		"accepted":  true,
		"event_id":  stored.EventID,
		"truncated": stored.Truncated,
	})
}

func (c *collector) handleContextSnapshot(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		w.WriteHeader(http.StatusMethodNotAllowed)
		return
	}
	if !c.authorized(r) {
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
		return
	}
	if !c.allowRequest(clientIP(r)) {
		writeJSON(w, http.StatusTooManyRequests, map[string]string{"error": "rate limit exceeded"})
		return
	}

	r.Body = http.MaxBytesReader(w, r.Body, defaultMaxSnapshotBytes)
	defer r.Body.Close()

	var req contextSnapshotRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid JSON"})
		return
	}

	if len(req.Messages) == 0 && strings.TrimSpace(req.ImageBase64) == "" {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "either messages or image_base64 is required"})
		return
	}

	// Immediately acknowledge client
	writeJSON(w, http.StatusAccepted, map[string]interface{}{
		"accepted":     true,
		"source_app":   req.SourceApp,
		"window_title": req.WindowTitle,
	})

	// Process in background
	go func() {
		if len(req.Messages) > 0 {
			modelName := req.Engine
			if modelName == "" {
				modelName = "local_ocr"
			}
			record := storedContextSnapshot{
				CapturedAt:  req.CapturedAt,
				ReceivedAt:  time.Now().UTC().Format(time.RFC3339Nano),
				SourceApp:   req.SourceApp,
				WindowTitle: req.WindowTitle,
				Model:       modelName,
				Messages:    req.Messages,
			}
			log.Printf("Received %d local OCR messages for %s (%s)", len(req.Messages), req.SourceApp, req.WindowTitle)
			if appendErr := c.appendContext(record); appendErr != nil {
				log.Printf("Failed to append context snapshot: %v", appendErr)
			}
			return
		}

		if isLikelyNonChatWindow(req.WindowTitle) {
			log.Printf("Skipping Vision AI for non-chat window: %s (%s)", req.SourceApp, req.WindowTitle)
			record := storedContextSnapshot{
				CapturedAt:  req.CapturedAt,
				ReceivedAt:  time.Now().UTC().Format(time.RFC3339Nano),
				SourceApp:   req.SourceApp,
				WindowTitle: req.WindowTitle,
				Model:       "skipped_non_chat",
				Messages:    nil,
			}
			if appendErr := c.appendContext(record); appendErr != nil {
				log.Printf("Failed to append context snapshot: %v", appendErr)
			}
			return
		}

		// Throttle concurrent Vision AI requests to avoid overwhelming upstream providers
		if c.visionSem != nil {
			c.visionSem <- struct{}{}
			defer func() { <-c.visionSem }()
		}

		msgs, raw, err := c.callVisionAI(req)
		record := storedContextSnapshot{
			CapturedAt:  req.CapturedAt,
			ReceivedAt:  time.Now().UTC().Format(time.RFC3339Nano),
			SourceApp:   req.SourceApp,
			WindowTitle: req.WindowTitle,
			Model:       c.cfg.AIModel,
			RawResponse: raw,
			Messages:    msgs,
		}
		if err != nil {
			log.Printf("Vision AI error for %s (%s): %v", req.SourceApp, req.WindowTitle, err)
			record.Error = err.Error()
		} else {
			log.Printf("Vision AI parsed %d messages for %s (%s)", len(msgs), req.SourceApp, req.WindowTitle)
		}
		if appendErr := c.appendContext(record); appendErr != nil {
			log.Printf("Failed to append context snapshot: %v", appendErr)
		}
	}()
}

func isLikelyNonChatWindow(title string) bool {
	t := strings.TrimSpace(title)
	if t == "" {
		return false
	}
	ignoredSubstrings := []string{
		"-文档",
		"腾讯文档",
		"微文档",
		"新建文档",
		"跳转列表",
		"选择聊天",
		"发起群聊",
	}
	for _, sub := range ignoredSubstrings {
		if strings.Contains(t, sub) {
			return true
		}
	}
	return false
}

func (c *collector) callVisionAI(snapshot contextSnapshotRequest) ([]storedDialogueMessage, string, error) {
	if c.cfg.AIAPIKey == "" {
		return nil, "", errors.New("QOLLECT_AI_API_KEY is not configured")
	}

	format := strings.ToLower(strings.TrimSpace(snapshot.ImageFormat))
	if format == "" {
		format = "jpeg"
	}
	dataURL := fmt.Sprintf("data:image/%s;base64,%s", format, snapshot.ImageBase64)

	var promptText string
	titleLower := strings.ToLower(snapshot.WindowTitle)
	if strings.Contains(titleLower, "预览") || strings.Contains(titleLower, "图片") || strings.Contains(titleLower, "preview") {
		promptText = "这是一张在微信/企业微信图片预览窗口中查看的图片或聊天记录长截图。请仔细识别并提取图片中的所有对话、消息或关键图文记录。严格以标准 JSON 数组输出每一条消息：[{\"speaker\": \"发送人昵称或内容归属\", \"role\": \"user或peer\", \"time\": \"时间（若无则填空字符串）\", \"content\": \"正文内容\"}]。若为聊天记录截图，请按对话条目提取；若为工作文档、架构或方案图，请提炼核心文字提取为条目。只输出纯 JSON，不要包含任何 markdown 解释或代码块标签。"
	} else {
		promptText = "这是一张微信/企业微信聊天窗口截图（可能包含常规会话或图片/聊天截图分享）。请识别其中的连续对话。忽略侧边栏、菜单和输入框，严格以标准 JSON 数组输出每一条消息：[{\"speaker\": \"发送人昵称（右边是我，左边是对方昵称）\", \"role\": \"user或peer\", \"time\": \"时间\", \"content\": \"正文\"}]。如果画面中有引用的聊天截图或转发内容，也请一并提取。只输出纯 JSON，不要包含任何 markdown 解释或代码块标签。"
	}

	reqPayload := openAIChatRequest{
		Model: c.cfg.AIModel,
		Messages: []openAIChatMessage{
			{
				Role: "user",
				Content: []openAIMessageContent{
					{
						Type:     "image_url",
						ImageURL: &openAIImageURL{URL: dataURL},
					},
					{
						Type: "text",
						Text: promptText,
					},
				},
			},
		},
	}

	bodyBytes, err := json.Marshal(reqPayload)
	if err != nil {
		return nil, "", err
	}

	endpoint := strings.TrimRight(c.cfg.AIBaseURL, "/") + "/chat/completions"
	client := &http.Client{Timeout: 120 * time.Second}

	var resp *http.Response
	var lastErr error
	maxRetries := 1

	for attempt := 0; attempt <= maxRetries; attempt++ {
		if attempt > 0 {
			time.Sleep(1500 * time.Millisecond)
			log.Printf("Retrying Vision AI request (attempt %d) for %s (%s)...", attempt+1, snapshot.SourceApp, snapshot.WindowTitle)
		}

		httpReq, reqErr := http.NewRequest(http.MethodPost, endpoint, bytes.NewReader(bodyBytes))
		if reqErr != nil {
			return nil, "", reqErr
		}
		httpReq.Header.Set("Content-Type", "application/json")
		httpReq.Header.Set("Authorization", "Bearer "+c.cfg.AIAPIKey)

		resp, err = client.Do(httpReq)
		if err != nil {
			lastErr = err
			continue
		}
		if resp.StatusCode == http.StatusOK {
			lastErr = nil
			break
		}

		respBytes, _ := io.ReadAll(resp.Body)
		resp.Body.Close()
		lastErr = fmt.Errorf("AI API returned status %d: %s", resp.StatusCode, string(respBytes))

		// Only retry for server busy (503), internal server error (500), bad gateway (502) or rate limit (429)
		if resp.StatusCode != http.StatusServiceUnavailable &&
			resp.StatusCode != http.StatusInternalServerError &&
			resp.StatusCode != http.StatusBadGateway &&
			resp.StatusCode != http.StatusTooManyRequests {
			return nil, string(respBytes), lastErr
		}
	}

	if lastErr != nil {
		return nil, "", lastErr
	}
	defer resp.Body.Close()

	respBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, "", err
	}

	var chatResp openAIChatResponse
	if err := json.Unmarshal(respBytes, &chatResp); err != nil {
		return nil, string(respBytes), err
	}

	if len(chatResp.Choices) == 0 {
		return nil, string(respBytes), errors.New("no response choices from AI")
	}

	rawContent := strings.TrimSpace(chatResp.Choices[0].Message.Content)
	cleanJSON := cleanDialogueJSON(rawContent)

	var messages []storedDialogueMessage
	if err := json.Unmarshal([]byte(cleanJSON), &messages); err != nil {
		return nil, rawContent, fmt.Errorf("parse dialogue json failed: %w", err)
	}

	return messages, rawContent, nil
}

func sanitizeJSONStringLiterals(raw string) string {
	var sb strings.Builder
	sb.Grow(len(raw) + 16)
	inString := false
	escaped := false

	for i := 0; i < len(raw); i++ {
		b := raw[i]
		if escaped {
			sb.WriteByte(b)
			escaped = false
			continue
		}
		if b == '\\' {
			sb.WriteByte(b)
			escaped = true
			continue
		}
		if b == '"' {
			inString = !inString
			sb.WriteByte(b)
			continue
		}
		if inString {
			switch b {
			case '\n':
				sb.WriteString(`\n`)
				continue
			case '\r':
				sb.WriteString(`\r`)
				continue
			case '\t':
				sb.WriteString(`\t`)
				continue
			}
		}
		sb.WriteByte(b)
	}
	return sb.String()
}

func cleanDialogueJSON(raw string) string {
	raw = strings.TrimSpace(raw)
	if start := strings.Index(raw, "["); start != -1 {
		if end := strings.LastIndex(raw, "]"); end > start {
			return sanitizeJSONStringLiterals(raw[start : end+1])
		}
	}
	if strings.HasPrefix(raw, "```") {
		idx := strings.Index(raw, "\n")
		if idx != -1 {
			raw = strings.TrimSpace(raw[idx+1:])
		}
		if lastIdx := strings.LastIndex(raw, "```"); lastIdx != -1 {
			raw = strings.TrimSpace(raw[:lastIdx])
		}
	}
	return sanitizeJSONStringLiterals(strings.TrimSpace(raw))
}

func clientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}

func (c *collector) allowRequest(ip string) bool {
	now := time.Now()
	cutoff := now.Add(-time.Minute)
	c.rateMu.Lock()
	defer c.rateMu.Unlock()
	entries := c.rate[ip]
	kept := entries[:0]
	for _, entry := range entries {
		if entry.After(cutoff) {
			kept = append(kept, entry)
		}
	}
	if len(kept) >= 120 {
		c.rate[ip] = kept
		return false
	}
	c.rate[ip] = append(kept, now)
	return true
}

func (c *collector) authorized(r *http.Request) bool {
	const prefix = "Bearer "
	value := r.Header.Get("Authorization")
	if !strings.HasPrefix(value, prefix) {
		return false
	}
	provided := strings.TrimSpace(strings.TrimPrefix(value, prefix))
	if provided == "" || len(provided) != len(c.cfg.Token) {
		return false
	}
	return subtle.ConstantTimeCompare([]byte(provided), []byte(c.cfg.Token)) == 1
}

func (c *collector) normalize(event incomingEvent) (storedEvent, error) {
	contentType := strings.ToLower(strings.TrimSpace(event.ContentType))
	if !isTextContentType(contentType) {
		return storedEvent{}, fmt.Errorf("unsupported content type %q", event.ContentType)
	}
	if event.Content == "" {
		return storedEvent{}, errors.New("content is required")
	}
	if !utf8.ValidString(event.Content) {
		return storedEvent{}, errors.New("content must be valid UTF-8")
	}

	eventID := strings.TrimSpace(event.EventID)
	if len(eventID) > maxEventIDLength {
		return storedEvent{}, fmt.Errorf("event_id is too long")
	}
	if eventID == "" {
		eventID = fmt.Sprintf("%d", time.Now().UnixNano())
	}

	originalChars := utf8.RuneCountInString(event.Content)
	digest := sha256.Sum256([]byte(event.Content))
	content := event.Content
	truncated := originalChars > c.cfg.MaxTextChars
	if truncated {
		content = string([]rune(event.Content)[:min(c.cfg.PreviewChars, originalChars)])
	}

	capturedAt := strings.TrimSpace(event.CapturedAt)
	if capturedAt == "" {
		capturedAt = time.Now().UTC().Format(time.RFC3339Nano)
	}

	return storedEvent{
		EventID:       eventID,
		CapturedAt:    capturedAt,
		ReceivedAt:    time.Now().UTC().Format(time.RFC3339Nano),
		ContentType:   contentType,
		Content:       content,
		AITags:        normalizeAITags(event.AITags),
		SourceApp:     strings.TrimSpace(event.SourceApp),
		OriginalChars: originalChars,
		ContentSHA256: hex.EncodeToString(digest[:]),
		Truncated:     truncated,
	}, nil
}

func normalizeAITags(tags []string) []string {
	result := make([]string, 0, len(tags))
	for _, tag := range tags {
		if normalized := strings.TrimSpace(tag); normalized != "" {
			result = append(result, normalized)
		}
	}
	return result
}

func isTextContentType(contentType string) bool {
	if contentType == "" || contentType == "text" || contentType == "text/plain" || contentType == "text/uri-list" || contentType == "link" || contentType == "url" || contentType == "markdown" || contentType == "code" {
		return true
	}
	return strings.HasPrefix(contentType, "text/")
}

func (c *collector) append(event storedEvent) error {
	day := time.Now().In(c.cfg.Location).Format("2006/01/02")
	path := filepath.Join(c.cfg.DataDir, day+".jsonl")

	c.mu.Lock()
	defer c.mu.Unlock()

	if err := os.MkdirAll(filepath.Dir(path), 0o750); err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return err
	}
	defer file.Close()

	encoder := json.NewEncoder(file)
	return encoder.Encode(event)
}

func (c *collector) appendContext(snapshot storedContextSnapshot) error {
	day := time.Now().In(c.cfg.Location).Format("2006/01/02")
	path := filepath.Join(c.cfg.ContextDir, day+".jsonl")

	c.mu.Lock()
	defer c.mu.Unlock()

	if err := os.MkdirAll(filepath.Dir(path), 0o750); err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return err
	}
	defer file.Close()

	encoder := json.NewEncoder(file)
	return encoder.Encode(snapshot)
}

func (c *collector) cleanupLoop() {
	ticker := time.NewTicker(cleanupInterval)
	defer ticker.Stop()
	for now := range ticker.C {
		c.pruneOldDays(now)
	}
}

func (c *collector) pruneOldDays(now time.Time) {
	cutoff := now.In(c.cfg.Location).AddDate(0, 0, -c.cfg.RetentionDays)
	pruneDir := func(baseDir string) {
		_ = filepath.Walk(baseDir, func(path string, info os.FileInfo, walkErr error) error {
			if walkErr != nil || info == nil || info.IsDir() || filepath.Ext(path) != ".jsonl" {
				return walkErr
			}
			relative, err := filepath.Rel(baseDir, path)
			if err != nil {
				return nil
			}
			datePath := strings.TrimSuffix(filepath.ToSlash(relative), ".jsonl")
			date, err := time.ParseInLocation("2006/01/02", datePath, c.cfg.Location)
			if err == nil && date.Before(cutoff) {
				if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
					log.Printf("remove expired file %s: %v", path, err)
				}
			}
			return nil
		})
	}

	pruneDir(c.cfg.DataDir)
	pruneDir(c.cfg.ContextDir)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func writeJSON(w http.ResponseWriter, status int, value interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}

func loggingMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		started := time.Now()
		next.ServeHTTP(w, r)
		log.Printf("%s %s (%s)", r.Method, r.URL.Path, time.Since(started).Round(time.Millisecond))
	})
}
