# Release e CI - Compilador Por do Sol

Este repositório possui dois workflows no GitHub Actions:

- `.github/workflows/ci.yml` (Integração Contínua)
- `.github/workflows/release.yml` (Publicação Automática de Release)

---

## 1. CI (Integração Contínua)

Executa compilação e suíte completa de testes em cada push/PR para:
- `windows-latest`
- `ubuntu-latest`
- `macos-latest`

---

## 2. Release Automática via Tag

Para publicar uma nova versão oficial no GitHub Releases:

```bash
git tag v0.1.4
git push origin v0.1.4
```

O workflow gera automaticamente os artefatos versionados por plataforma:

- `compilador-portugues-v<versao>-windows-x64.zip` (com `compilador.exe` e `interpretador.exe`)
- `compilador-portugues-v<versao>-linux-x64.tar.gz` (com `compilador` e `interpretador`)
- `compilador-portugues-v<versao>-macos-x64.tar.gz` (com `compilador` e `interpretador`)

E também:
- Checksums `.sha256` individuais
- `SHA256SUMS.txt` consolidado
- Release Notes geradas automaticamente

---

## 3. Gerar Release Localmente

### Windows (PowerShell)
```powershell
.\scripts\build-release.ps1 -Version "0.1.4"
```

### Linux / macOS (Bash)
```bash
chmod +x ./scripts/build-release.sh
./scripts/build-release.sh "0.1.4"
```

Os artefatos finais são colocados no diretório `dist/`.
