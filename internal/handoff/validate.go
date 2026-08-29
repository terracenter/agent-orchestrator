package handoff

import (
	"regexp"
	"strings"
)

var volatilePatterns = []*regexp.Regexp{
	regexp.MustCompile(`\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}`),
	regexp.MustCompile(`(?i)session[_ -]?id`),
	regexp.MustCompile(`(?i)timestamp`),
}

func ValidateCacheableTemplate(content string) []string {
	var warnings []string
	for _, block := range []string{"contexto_estatico", "contexto_estable"} {
		body := betweenTags(content, block)
		if body == "" {
			continue
		}
		for _, pattern := range volatilePatterns {
			if pattern.MatchString(body) {
				warnings = append(warnings, "dato volatil en "+block)
				break
			}
		}
	}
	return warnings
}

func betweenTags(content, tag string) string {
	open := "<" + tag + ">"
	close := "</" + tag + ">"
	start := strings.Index(content, open)
	if start < 0 {
		return ""
	}
	start += len(open)
	end := strings.Index(content[start:], close)
	if end < 0 {
		return ""
	}
	return content[start : start+end]
}
