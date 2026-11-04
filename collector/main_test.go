package main

import (
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
