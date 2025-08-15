# Build, Alvos e Testes

## Compilar o compilador
```powershell
cargo build --release
```

## Rodar exemplos (bytecode)
```powershell
cargo test --test examples_test -- --nocapture
```

## Gerar IR LLVM dos exemplos
```powershell
cargo test --test llvm_examples_test -- --nocapture
```

## Rodar toda suíte
```powershell
cargo test -- --nocapture
```

Dicas PowerShell: para setar variáveis de ambiente por sessão, use:
```powershell
$env:RUST_BACKTRACE = '1'
```
