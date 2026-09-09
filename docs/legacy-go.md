# Codigo Go archivado

Desde #194, `orq` se distribuye exclusivamente desde el crate Rust
`orq-agent`. Las rutas operativas (`scripts/install.sh`, `make build`,
`make install`, Docker y CI) no construyen ni instalan codigo Go.

Los directorios `cmd/` e `internal/` permanecen temporalmente como referencia
de paridad durante la migracion por slices descrita en
[`matriz-paridad-rust.md`](matriz-paridad-rust.md). No son una interfaz
soportada ni deben usarse para instalar un binario `orq`.

La eliminacion fisica del codigo Go depende de completar los comandos Rust
pendientes de la matriz de paridad. Hasta entonces, las correcciones al codigo
archivado se limitan a preservar la referencia historica o facilitar su retiro.
