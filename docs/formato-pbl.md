# Especificação do Formato .pbl (Biblioteca Por do Sol)

O formato `.pbl` (Por do Sol Biblioteca) é o formato moderno de bibliotecas compiladas na linguagem Por do Sol. 
Diferente do formato legado `.pbc` que contém apenas bytecode de forma sequencial, o `.pbl` é dividido em duas seções distintas, o que permite o uso de _Reference Assemblies_ (metadados para verificação de tipos em tempo de compilação sem precisar carregar todo o código em memória).

## Estrutura do Arquivo

O arquivo deve conter as seções `[MANIFESTO]` e `[BYTECODE]`.

### Seção de Manifesto

A seção `[MANIFESTO]` define os metadados de todas as classes, métodos, campos e propriedades exportados pela biblioteca. O compilador usa **apenas** esta seção durante a verificação semântica e de tipos (Type Checking).

A estrutura de metadados suporta as seguintes diretivas (linhas começando com o nome da diretiva e separadas por espaço):

- `DEFINE_CLASS <fqn> <nome_pai>`
  Define uma classe de instância normal.
  * `<fqn>`: Nome totalmente qualificado (Full Qualified Name) da classe. (Ex: `Sistema.Excecoes.ErroLocal`)
  * `<nome_pai>`: FQN da classe pai para herança, ou `NULO` caso não herde de ninguém.

- `DEFINE_STATIC_CLASS <fqn>`
  Define uma classe puramente estática. 

- `PROPERTY <fqn_classe> <nome> <tipo>`
  Define uma propriedade em uma classe (estática ou de instância).
  * `<fqn_classe>`: Classe à qual a propriedade pertence.
  * `<nome>`: Nome da propriedade.
  * `<tipo>`: Tipo de dado esperado.

- `FIELD <fqn_classe> <nome> <tipo>`
  Define um campo (variável de classe/instância).

- `DEFINE_STATIC_NATIVE_METHOD <fqn_classe> <nome_metodo> <tipo_retorno> <chave_nativa> [param1_tipo:param1_nome ...]`
  Define um método nativo estático. 
  * `<chave_nativa>` é a chave de despacho usada no `interpretador.rs` (ex: `Console::EscreverLinha`).
  * Parâmetros seguem o formato `tipo:nome` separados por espaços.

- `DEFINE_NATIVE_METHOD <fqn_classe> <nome_metodo> <tipo_retorno> <chave_nativa> [param1_tipo:param1_nome ...]`
  Define um método nativo de instância. Possui o mesmo formato da versão estática.

- `DEFINE_STATIC_METHOD <fqn_classe> <nome_metodo> <tipo_retorno> <num_parametros> [param1_tipo:param1_nome ...]`
  Define a assinatura de um método estático em bytecode de usuário.

- `DEFINE_METHOD <fqn_classe> <nome_metodo> <tipo_retorno> <num_parametros> [param1_tipo:param1_nome ...]`
  Define a assinatura de um método de instância normal.

**Exemplo de Manifesto:**
```
[MANIFESTO]
nome=Sistema.IO
versao=1.0

DEFINE_STATIC_CLASS Sistema.IO.Arquivo
DEFINE_STATIC_NATIVE_METHOD Sistema.IO.Arquivo LerTexto Texto Arquivo::LerTexto Texto:caminho
DEFINE_STATIC_NATIVE_METHOD Sistema.IO.Arquivo EscreverTexto Nulo Arquivo::EscreverTexto Texto:caminho Texto:conteudo
```

### Seção de Bytecode

A seção `[BYTECODE]` (ou `[PBL]`) abriga o corpo e a lógica executável das classes e métodos normais que não são implementados nativamente pelo runtime. 

Sua estrutura é exatamente idêntica ao que existe hoje no formato legado `.pbc`. O interpretador carrega as instruções a partir dessa seção quando está em modo de execução.

**Exemplo de Bytecode:**
```
[BYTECODE]
DEFINE_CLASS Usuario NULO
DEFINE_FUNCTION Usuario.construtor 3
LOAD_VAR este
LOAD_VAR nome
SET_PROPERTY nome
RETURN
END_CLASS
```

## Compatibilidade Retroativa

O runtime permanece capaz de rodar arquivos `.pbc` legados sem problemas, embora para uso como referência no compilador, o formato `.pbl` apresente expressivas vantagens de desempenho de IO por dispensar a leitura de bytes de execução.