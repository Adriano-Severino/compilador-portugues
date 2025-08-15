# Suporte a Enumeração

- Palavra-chave: `enumeração`
- Sintaxe:
```
espaco Testes {
  enumeração Cor {
    Vermelho, Verde, Azul
  }
}
```
- Uso:
```
Cor x = Cor.Verde;
se (x == Cor.Verde) { imprima(123); }
```
- Regras de tipo:
  - Membros de `Cor` têm tipo `Cor` (não inteiro).
  - Atribuições e comparações só são válidas entre o mesmo enum (ex.: `Cor` com `Cor`).
  - Em impressão, o valor é exibido como inteiro (índice do membro a partir de 0).
- Backends:
  - LLVM: enums são mapeados para i32; membros geram constantes inteiras; comparações via `icmp`.
  - Bytecode: membros carregam `LOAD_CONST_INT` e comparam como inteiros.

Arquivos de teste:
- `teste_enum.pr` (positivo)
- `teste_enum_neg_diferentes.pr` (mistura de enums diferentes — deve falhar)
- `teste_enum_neg_membro_invalido.pr` (membro inexistente — deve falhar)
