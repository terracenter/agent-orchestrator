package handoff

import "testing"

func TestValidateCacheableTemplateWarnsVolatileStatic(t *testing.T) {
	warnings := ValidateCacheableTemplate(`<contexto_estatico>
session_id=abc
</contexto_estatico>
<contexto_dinamico>
Generado: 2026-08-29T03:00:00Z
</contexto_dinamico>`)
	if len(warnings) != 1 {
		t.Fatalf("expected one warning, got %v", warnings)
	}
}

func TestValidateCacheableTemplateAllowsVolatileDynamic(t *testing.T) {
	warnings := ValidateCacheableTemplate(`<contexto_estatico>
Reglas permanentes
</contexto_estatico>
<contexto_dinamico>
Generado: 2026-08-29T03:00:00Z
</contexto_dinamico>`)
	if len(warnings) != 0 {
		t.Fatalf("expected no warning, got %v", warnings)
	}
}
