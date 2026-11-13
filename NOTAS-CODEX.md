# Notas de implementação — funcionalidades

Todas as funcionalidades usam dados locais no diretório do Verso Ink. A geração das imagens usa um perfil temporário isolado e não altera os dados do usuário. Antes de cada commit, foram executados `cargo build --release` e `cargo test --release`; as telas foram geradas pelo teste ignorado `todas_as_telas` e abertas para conferência. Não foi executado `cargo fmt`, deploy ou push.

Para repetir os prints:

```sh
mkdir -p /tmp/verso-shots
VERSO_INK_SHOTS=/tmp/verso-shots cargo test --release todas_as_telas -- --ignored --test-threads=1
```

## 1. Reversi

- **O que foi feito:** tabuleiro 8×8, lances possíveis, viradas, passagem automática, placar, dois jogadores e CPU em três níveis. O nível difícil usa busca minimax e valores posicionais.
- **Onde está:** `src/reversi.rs`, registro em `src/apps.rs`, `src/minigame.rs` e tutorial em `src/tutorial.rs`.
- **Testes:** início, captura em várias direções, passagem, fim e escolha legal da CPU.
- **Prints conferidos:** menu, partida e fim.
- **Limitações:** a força da CPU foi verificada por regras e exemplos, sem torneio exaustivo.

## 2. Go

- **O que foi feito:** Go 9×9, contra CPU ou a dois, com captura, suicídio proibido, ko simples, passagem e contagem por área com komi de 6,5. A CPU usa simulações leves.
- **Onde está:** `src/go.rs`, registros dos jogos e tutorial.
- **Testes:** captura, suicídio, ko, passagem, território neutro, komi e legalidade da jogada da CPU.
- **Prints conferidos:** menu, partida e fim.
- **Limitações:** na contagem, as pedras que permanecem no tabuleiro são consideradas vivas; grupos presos devem ser capturados antes das duas passagens. Não há etapa de marcação de grupos mortos.

## 3. Palavras cruzadas

- **O que foi feito:** grade diária reproduzível de aproximadamente 11×11, escolha de palavra por toque, entrada pelo teclado, conferência e revelação de letra; progresso salvo por dia.
- **Onde está:** `src/cruzadas.rs`, `assets/cruzadas.txt` (691 pares de palavra e dica), dados em `data/cruzadas/`.
- **Testes:** lista sem duplicatas, geração conectada e reproduzível, recuperação de progresso e troca da grade ao mudar o dia.
- **Prints conferidos:** menu, grade, entrada e conclusão.
- **Limitações:** a entrada é feita por palavra no teclado do aplicativo; a lista e a grade usam grafia sem acentos para manter a correspondência das casas.

## 4. Puzzle do dia no Xadrez

- **O que foi feito:** escolha diária de erros analisados em partidas salvas contra treinadores, posição antes do erro, comparação com o melhor lance, explicação e sequência diária. Há tela vazia quando não existem candidatos.
- **Onde está:** `src/puzzle_dia.rs`, integração em `src/app.rs`, registros em `data/xadrez/puzzle/`.
- **Testes:** seleção de erros válidos, reconstrução e legalidade da posição e do lance, seleção estável, sequência e tratamento de falhas de leitura e escrita.
- **Prints conferidos:** tela vazia, puzzle, explicação/fim e falha ao salvar.
- **Limitações:** depende de partidas com erros analisados e `best_uci` válido; partidas sem análise não fornecem puzzles.

## 5. Geografia

- **O que foi feito:** quatro lições offline, 24 perguntas sobre capitais e localização, uma pergunta sobre bandeira descrita em texto, mapa simplificado e integração ao XP e à ofensiva do Aprender.
- **Onde está:** `src/geografia.rs`, `src/aprender.rs`, `data/geografia.txt` e `data/aprender.txt`.
- **Testes:** alternativas e respostas, repetição de erros, etapas da lição e registro real de XP em perfil isolado.
- **Prints conferidos:** menu, introdução, pergunta, resposta e fim.
- **Limitações:** conteúdo inicial fixo; o mapa é esquemático e não representa fronteiras ou escala.

## 6. Matemática mental

- **O que foi feito:** treino por nível com tabuada, somas de dois dígitos, porcentagem e frações; cronômetro, XP e ofensiva do Aprender.
- **Onde está:** `src/matematica.rs` e integração em `src/aprender.rs`.
- **Testes:** geração de contas e alternativas, contagem de respostas, cronômetro e persistência de XP/ofensiva em perfil isolado.
- **Prints conferidos:** menu, pergunta, resposta e fim.
- **Limitações:** contas são geradas offline a partir dos níveis definidos, sem calculadora durante o treino.

## 7. Diário do dia

- **O que foi feito:** 120 perguntas, uma escolhida por data, escrita pelo teclado, gravação da resposta e consulta paginada das respostas anteriores.
- **Onde está:** `src/diario.rs`, `assets/diario-perguntas.txt`, acesso em Mais e arquivos em `data/diario/`.
- **Testes:** estabilidade da pergunta e data, persistência e ordenação, quebra de texto e preservação dos dados em falha de leitura.
- **Prints conferidos:** início, teclado, resposta, histórico e resposta antiga.
- **Limitações:** diário local e sem sincronização própria; em caso de falha ao salvar, a edição permanece disponível para nova tentativa enquanto a tela estiver aberta.

## 8. Flashcards

- **O que foi feito:** leitura de baralhos de texto do Anki (`frente<TAB>verso`), revisão com Errei/Difícil/Bom/Fácil e repetição espaçada SM-2 simplificada. O conversor extrai notas de `.apkg` e remove HTML.
- **Onde está:** `src/flashcards.rs`, `tools/anki-para-texto.py`, `tools/test_anki_para_texto.py`; baralhos em `data/flashcards/*.txt` e progresso em `data/flashcards-progresso.txt`.
- **Testes:** importação, linhas inválidas, intervalos, persistência e reordenação dos cartões; testes Python do conversor.
- **Prints conferidos:** vazio, baralhos, frente, verso e fim.
- **Limitações:** o conversor aceita `collection.anki2` e `collection.anki21`, não arquivos apenas no formato compactado `collection.anki21b`; mudar nome do arquivo ou texto do cartão altera sua identidade e reinicia seu progresso.

## 9. Hábitos e tarefas

- **O que foi feito:** criação, marcação e exclusão pelo teclado, sequência diária de hábitos, tarefas rápidas e resumo na Início. A exclusão pede confirmação.
- **Onde está:** `src/organizacao.rs`, integração em Mais e Início; hábitos e itens antigos em `data/dia.txt`, marcações em `data/dias/` e tarefas rápidas em `data/tarefas_rapidas.txt`.
- **Testes:** criação, exclusão seletiva, preservação de linhas, marcação, persistência de tarefas e falhas de leitura e escrita.
- **Prints conferidos:** vazio, resumo, lista, marcação, teclado, exclusão e confirmação.
- **Limitações:** tarefas antigas de `dia.txt` preservam a marcação diária já existente; as tarefas rápidas novas conservam a conclusão entre dias.

## 10. Calculadora e conversor

- **O que foi feito:** calculadora com precedência, parênteses, porcentagem e histórico; conversões de comprimento, massa, temperatura, volume, tempo e moedas; gorjeta e divisão da conta em centavos.
- **Onde está:** `src/calculadora.rs`; histórico em `data/calculadora-historico.txt` e cotações em `data/cotacoes.txt`.
- **Testes:** precedência, erros, unidades, cotações, precisão, arredondamento e divisão da conta.
- **Prints conferidos:** menu, conta, histórico, categorias, conversão, moedas sem cotação e gorjeta.
- **Limitações:** `%` é operador pós-fixo (`10% = 0,1`). A cotação deve ser preenchida externamente em reais por unidade, por exemplo `USD=5.20`; a ponte de cotações não foi alterada.

## 11. Lembretes

- **O que foi feito:** criação de lembretes avulsos com hora e texto em Mais, aviso em tela cheia com paginação e confirmação, preservação dos lembretes recorrentes já existentes e programação do próximo horário no RTC antes de suspender.
- **Onde está:** `src/lembretes.rs`, `src/lembretes_ui.rs`, `src/day.rs`, `src/today_ui.rs`, `src/main.rs` e `src/app.rs`; dados em `data/lembretes_avulsos.txt` e nos arquivos diários existentes.
- **Testes:** regras de horário e próxima ocorrência, avisos simultâneos, persistência segura e falhas de leitura/escrita; build e suíte release concluídos.
- **Prints conferidos:** vazio, horário, lista e avisos em tela cheia, incluindo texto paginado.
- **Limitações:** lembretes criados pela tela são avulsos para a próxima ocorrência da hora; o dispositivo não tem som. Confirmações recorrentes antigas com o mesmo texto compartilham a chave existente. O despertar físico via RTC e a resposta do gerenciador de energia a avisos pendentes ou falhas precisam ser testados no Kindle; no PC sem RTC não há programação de hardware.

## 12. Caderno à mão

- **O que foi feito:** folha para traço contínuo por toque, borracha, limpar e múltiplas páginas em PNG.
- **Onde está:** `src/caderno.rs`, integração de eventos e tela do aplicativo; páginas em `data/caderno/`.
- **Testes:** interpolação do traço, borracha, limites, troca de páginas e persistência/reabertura dos PNGs; teste de telas com eventos de toque.
- **Prints conferidos:** folha vazia, desenho, borracha, nova página, página anterior, limpeza e menu Mais.
- **Limitações:** o comportamento real do painel de toque e das atualizações e-ink deve ser validado no Kindle. Em falha de gravação, é necessário manter o desenho em memória e tentar salvar novamente.

### Dicionário: idioma em TXT e MOBI

TXT não informa idioma e o leitor atual de MOBI não expõe um metadado de idioma confiável. Nesses formatos, a consulta procura primeiro em português e depois em inglês; por isso uma palavra inglesa que também exista em português pode receber o verbete português. No EPUB, o idioma declarado em `dc:language` determina a prioridade.

### 1. Dicionário no leitor — implementação parcial

- **O que foi feito:** consulta por toque longo, busca binária nos TSVs, variantes simples, prioridade de idioma em EPUB, verbete paginado e aviso de grafia antiga uma vez. A palavra original é preservada quando a paginação a divide.
- **Onde está:** `src/dicionario.rs`, `src/reader.rs`, `src/app.rs`; aviso em `data/dicionario-aviso.txt`.
- **Testes:** `cargo build --release` sem avisos; `cargo test --release` com 162 aprovados e 19 ignorados; `todas_as_telas` passou. A busca foi testada também nos dois TSVs locais.
- **Print conferido:** `leitor-dicionario`, no framebuffer 1072×1448, com verbete, aviso e botão de fechar.
- **Limitações:** TXT e MOBI não têm identificação automática de idioma nesta implementação, conforme a decisão acima. Os TSVs são ignorados pelo Git: foram copiados do outro checkout para `assets/dict` nesta máquina e precisam estar em `<home>/dict` no Kindle. O gesto e a atualização da tela não foram testados no aparelho.
- **Commit:** não criado: os metadados Git desta worktree ficam fora das raízes graváveis e a criação de `index.lock` falhou com sistema de arquivos somente para leitura.

### Itens 2 a 18

Ainda não implementados nem verificados nesta worktree.
## 13. Domínio

- **O que foi feito:** mapa de 16 territórios com padrões, reforço, combate determinístico, movimento, desfazer, fases, desafio diário e CPU para um ou dois adversários.
- **Onde está:** `src/dominio.rs`, registro em Jogar e tutorial; progresso em `data/fases/dominio.txt` e `data/dominio-dia.txt`.
- **Testes:** combate, reforços, CPU, desfazer, sequência diária e solução de 1.000 fases; build e suíte release sem avisos ou falhas.
- **Prints conferidos:** Jogar, menu, partida e fim, gerados pelo teste `todas_as_telas`.
- **Limitações:** o desafio diário percorre 20 configurações e se repete; as fases têm limite de três turnos. Teste físico no Kindle ainda é necessário. O sprite específico não estava disponível e será usado quando adicionado.

## 14. Detetive

- **O que foi feito:** caso diário com cinco suspeitos, depoimentos, locais, pistas, caderno automático, doze horas de investigação, acusação em três partes, explicação e sequência.
- **Onde está:** `src/detetive.rs`, listas em `assets/detetive.txt`, registro em Jogar e tutorial; progresso em `data/detetive.txt`.
- **Testes:** geração de 200 casos e enumeração das soluções compatíveis com as pistas, limite de horas, alvos ao esgotar o tempo e reprodução pela data; suíte release aprovada.
- **Prints conferidos:** Jogar, menu, introdução, suspeitos, pista, caderno, acusação e fim, gerados pelo teste `todas_as_telas`.
- **Limitações:** os casos compartilham a estrutura de roubo de uma joia, com personagens, locais e elementos variáveis; a acusação encerra o caso do dia. O sprite específico não estava disponível. É necessário validar toque e e-ink no Kindle físico.

## Ambiente desta execução

- Esta pasta estava na branch `codex/jogos2`; `codex/funcionalidades` está em outra worktree e não foi alterada.
- Os commits solicitados não puderam ser criados: o diretório Git compartilhado foi montado somente para leitura (`index.lock`: `Read-only file system`). As alterações permanecem na worktree.
- A atualização automática do Brain não rodou: o `pwsh` instalado recusou a execução por uma falha do ambiente snap-confine.
