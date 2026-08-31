# Checklist de Release - Compilador Por do Sol

## Antes da Tag

1. [ ] Confirmar versão no `Cargo.toml` (`compilador-portugues`).
2. [ ] Executar formatação e testes:
   ```bash
   cargo fmt --check
   cargo test
   ```
3. [ ] Testar compilação dos exemplos:
   ```bash
   cargo run --release --bin compilador -- exemplos/teste.pr --target=bytecode
   cargo run --release --bin interpretador -- build/teste.pbc
   ```
4. [ ] Atualizar documentação se novos recursos da linguagem foram adicionados.

## Publicação

1. [ ] Criar tag com prefixo `v` correspondente à versão do `Cargo.toml`:
   ```bash
   git tag v0.1.4
   git push origin v0.1.4
   ```
2. [ ] Acompanhar a execução do workflow `Release` no GitHub Actions.
3. [ ] Validar que os seguintes artefatos foram anexados:
   - `compilador-portugues-v<versao>-windows-x64.zip`
   - `compilador-portugues-v<versao>-linux-x64.tar.gz`
   - `compilador-portugues-v<versao>-macos-x64.tar.gz`
   - `.sha256` correspondentes
   - `SHA256SUMS.txt`

## Pós-Release

1. [ ] Baixar os binários gerados e testar a execução em ambiente limpo:
   ```bash
   compilador --help
   interpretador --help
   ```
