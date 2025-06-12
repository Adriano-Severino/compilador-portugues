param(
    [Parameter(Mandatory=$true)]
    [string]$Programa
)

Write-Host "Compilando $Programa.pr..."

# Usar o compilador já construído
& "./target/release/compilador-portugues" "$Programa.pr"

# Gerar executável diretamente
& clang -O3 "$Programa.ll" -o "$Programa.exe"

Write-Host "Pronto! Execute com: ./$Programa.exe"
