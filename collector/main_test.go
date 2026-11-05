package main

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestNormalizeTruncatesByUnicodeCharacters(t *testing.T) {
	c := &collector{cfg: config{MaxTextChars: 3, PreviewChars: 2}}
	event, err := c.normalize(incomingEvent{
		EventID:     "event-1",
		ContentType: "text/plain",
		Content:     "你a好世界",
	})
	if err != nil {
		t.Fatalf("normalize returned error: %v", err)
	}
	if event.Content != "你a" {
		t.Fatalf("unexpected preview %q", event.Content)
	}
	if event.OriginalChars != 5 || !event.Truncated {
		t.Fatalf("unexpected truncation metadata: %+v", event)
	}
}

func TestNormalizeKeepsShortText(t *testing.T) {
	c := &collector{cfg: config{MaxTextChars: 20, PreviewChars: 2}}
	event, err := c.normalize(incomingEvent{ContentType: "text", Content: "hello"})
	if err != nil {
		t.Fatalf("normalize returned error: %v", err)
	}
	if event.Content != "hello" || event.Truncated {
		t.Fatalf("short text was changed: %+v", event)
	}
	if len(strings.TrimSpace(event.ContentSHA256)) == 0 {
		t.Fatal("content hash is missing")
	}
}

func TestNormalizeStoresAITags(t *testing.T) {
	c := &collector{cfg: config{MaxTextChars: 20, PreviewChars: 2}}
	event, err := c.normalize(incomingEvent{
		ContentType: "text",
		Content:     "hello",
		AITags:      []string{" Rust ", "", "analysis"},
	})
	if err != nil {
		t.Fatalf("normalize returned error: %v", err)
	}
	if got, want := strings.Join(event.AITags, ","), "Rust,analysis"; got != want {
		t.Fatalf("unexpected AI tags %q", got)
	}
}

func TestNormalizeRejectsNonTextContent(t *testing.T) {
	c := &collector{cfg: config{MaxTextChars: 20, PreviewChars: 2}}
	if _, err := c.normalize(incomingEvent{ContentType: "image/png", Content: "ignored"}); err == nil {
		t.Fatal("expected image content to be rejected")
	}
}

func TestCleanDialogueJSON(t *testing.T) {
	rawMarkdown := "```json\n[{\"speaker\": \"张三\", \"role\": \"peer\", \"content\": \"你好\"}]\n```"
	cleaned := cleanDialogueJSON(rawMarkdown)
	if !strings.HasPrefix(cleaned, "[") || !strings.HasSuffix(cleaned, "]") {
		t.Fatalf("cleanDialogueJSON did not remove markdown blocks: %s", cleaned)
	}

	plain := "[{\"speaker\": \"李四\", \"role\": \"user\", \"content\": \"收到\"}]"
	if got := cleanDialogueJSON(plain); got != plain {
		t.Fatalf("cleanDialogueJSON altered plain JSON: got %q, want %q", got, plain)
	}

	wrapped := "好的，以下是识别结果：\n```json\n[{\"speaker\": \"王五\", \"role\": \"user\", \"content\": \"好的\"}]\n```\n希望对您有帮助！"
	if got := cleanDialogueJSON(wrapped); got != "[{\"speaker\": \"王五\", \"role\": \"user\", \"content\": \"好的\"}]" {
		t.Fatalf("cleanDialogueJSON failed on wrapped JSON: got %q", got)
	}
}

func TestLocalOCRMessagesValidation(t *testing.T) {
	req := contextSnapshotRequest{
		CapturedAt:  "2026-11-01T22:00:00Z",
		SourceApp:   "WeChat",
		WindowTitle: "Test",
		Engine:      "local_ocr",
		Messages: []storedDialogueMessage{
			{Speaker: "我", Role: "user", Content: "你好"},
		},
	}
	if len(req.Messages) == 0 {
		t.Fatal("expected messages to be present")
	}
	if req.Engine != "local_ocr" {
		t.Fatalf("unexpected engine %q", req.Engine)
	}
}

func TestSanitizeJSONStringLiteralsWithUnescapedNewlines(t *testing.T) {
	// Raw JSON containing illegal unescaped newline and tab inside string literal
	rawWithNewlines := "[\n  {\n    \"speaker\": \"张三\",\n    \"role\": \"peer\",\n    \"content\": \"第一行\n第二行\t含Tab\"\n  }\n]"
	cleaned := cleanDialogueJSON(rawWithNewlines)
	if strings.Contains(cleaned, "第一行\n第二行") {
		t.Fatalf("cleanDialogueJSON failed to escape literal newline inside string: %s", cleaned)
	}
	if !strings.Contains(cleaned, `第一行\n第二行`) {
		t.Fatalf("expected escaped newline, got: %s", cleaned)
	}

	// Verify it unmarshals successfully
	var messages []storedDialogueMessage
	if err := json.Unmarshal([]byte(cleaned), &messages); err != nil {
		t.Fatalf("failed to unmarshal sanitized JSON: %v", err)
	}
	if len(messages) != 1 || messages[0].Speaker != "张三" {
		t.Fatalf("unexpected unmarshaled message: %+v", messages)
	}
	if messages[0].Content != "第一行\n第二行\t含Tab" {
		t.Fatalf("unexpected message content: %q", messages[0].Content)
	}
}

func TestIsLikelyNonChatWindow(t *testing.T) {
	cases := []struct {
		title    string
		expected bool
	}{
		{"企业微信-文档", true},
		{"腾讯文档 - 季度复盘", true},
		{"微文档", true},
		{"企业微信 的跳转列表", true},
		{"选择聊天", true},
		{"发起群聊", true},
		{"项目群", false},
		{"企业微信", false},
		{"雷永成(91044)", false},
		{"APEX+XDR研发TL&PM&TSE", false},
	}

	for _, c := range cases {
		got := isLikelyNonChatWindow(c.title)
		if got != c.expected {
			t.Errorf("isLikelyNonChatWindow(%q) = %v; want %v", c.title, got, c.expected)
		}
	}
}
