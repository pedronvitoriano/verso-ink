Você trabalha no repositório verso-ink (esta pasta, branch codex/funcionalidades): um sistema próprio em Rust para Kindle Paperwhite (e-ink 1072×1448, tons de cinza, toque, sem som). Usuário único: Pedro, brasileiro, exigente com visual e UX. Tudo em português do Brasil.

## Regras do projeto (obrigatórias)
- NUNCA rode `cargo fmt` (o projeto não usa rustfmt; reformataria tudo). Escreva no estilo do código existente.
- Não rode deploy, não mexa em servidor, não faça push. Commits locais nesta branch, um por funcionalidade, mensagem em português descrevendo o que mudou.
- Antes de cada commit: `cargo build --release` sem nenhum warning e `cargo test --release` verde. Cada funcionalidade nova ganha testes de regras/lógica (ex.: jogadas legais, pontuação, parse de arquivo).
- Prints: o teste ignorado `todas_as_telas` (em src/app.rs) gera prints de todas as telas com `VERSO_INK_SHOTS=/pasta cargo test --release todas_as_telas -- --ignored --test-threads=1`. Acrescente prints das telas novas (menu, partida/uso, fim) e CONFIRA cada print abrindo a imagem: margens de 72 px, nada cortado ou sobreposto, texto legível, nenhum rótulo técnico.
- Siga os padrões existentes: minijogos implementam o trait `MiniGame` (veja src/minigame.rs, src/velha.rs como exemplo recente e completo: menu com `minigame::draw_menu`, `menu_art`, `header`, `pill`, `bottom_button`, CPU com `cpu_turn`/`cpu_move`, registro em `apps::GAMES`, `GAME_GROUPS`, `minigame::make` e `tutorial::pages`). Telas do app ficam em `Screen` (src/app.rs) com desenho, alvos de toque e handler de toque; veja como Aprender (src/estudo.rs, src/aprender.rs) e Mais (src/apps.rs) organizam módulos.
- Estilo visual: cartões brancos com borda preta grossa e sombra chapada (`ui::card`, `ui::button`), tipografia grande e ousada (Face::Display), ícones existentes em assets/sprites. Sem caracteres que a fonte não tem (teste: ✦ não existe).
- Dados do usuário ficam em `crate::home_dir().join("data/...")` em texto simples.
- Se travar numa decisão, escolha a opção mais simples e segura e anote no fim do arquivo NOTAS-CODEX.md (decisão + motivo).
- Nunca encerre "aguardando": trabalhe até terminar tudo da lista.

## O que implementar (nesta ordem, um commit por item)

O item 1 (dicionário no leitor) JÁ ESTÁ FEITO e commitado: comece pelo 2. Se um commit falhar por permissão, NÃO PARE: anote no NOTAS-CODEX.md e siga para o próximo item até o 18.

Leitura (o leitor próprio está em src/reader.rs; o Verso é a ponte de chat em src/chat.rs, `crate::chat::send(None, prompt, Some(contexto))`, com respostas lidas por `crate::chat::open`):
1. Dicionário no leitor: segurar (toque longo) uma palavra mostra um cartão com o verbete. Dicionários em `<home>/dict/pt.tsv` e `<home>/dict/en-pt.tsv` (no Kindle `/mnt/us/verso-ink/dict`; no PC `assets/dict`): uma linha por verbete "chave<TAB>palavra<TAB>definição", ordenadas pela chave (minúsculas sem acento) — busca binária lendo o arquivo por posição (não carregar 12 MB na memória). Tente a palavra, depois sem plural/flexões simples (s, es, ões→ão, inho/inha→o/a) e, se o livro estiver em inglês ou a palavra não existir em pt, o en-pt. Aviso discreto de que o dicionário português é de 1913 (grafia antiga) só uma vez.
2. Vocabulário: toda palavra consultada vira um cartão de flashcard (o sistema de flashcards já existe: data/flashcards) num baralho "Vocabulário", frente a palavra + frase do livro, verso a definição curta.
3. Estatísticas de leitura: tempo lido por dia (registre sessões de leitura do leitor), ritmo (páginas/min) e "faltam ~N min para o fim do capítulo / do livro" no rodapé do leitor e numa tela de estatísticas (semana em barras).
4. Verso no leitor (menu do leitor): "Resumo até aqui" (sem spoiler: mande só o texto já lido, até onde cabe, pedindo resumo do que aconteceu), "Pergunte ao livro" (pergunta livre respondida só com o já lido), "Explicar trecho" (selecionar trecho: toque longo no início e arraste/toque no fim; mande o trecho com contexto) e "Clube do livro" (ao terminar um capítulo, 2 ou 3 perguntas para pensar; a resposta da pessoa pode ser enviada ao Verso para conversar).
5. Recomendações: em Ler, "Próximo livro" pede ao Verso 3 sugestões pelo que foi lido e destacado (títulos e destaques), com uma linha do porquê.

Ponte (a VPS já tem: GET /agenda → {"configured":false} ou lista de eventos {inicio,fim,titulo,local,dia:"hoje|amanha"}; GET /cotacoes → {"USD":5.62,"EUR":...,"GBP":...,"ARS":...,"quando":ISO}; POST /backup corpo tar.gz até 50 MB, GET /backup lista; autenticação igual às outras chamadas da ponte em src/chat.rs/painel.rs):
6. Cotações: ao sincronizar (onde o clima é atualizado), buscar /cotacoes e gravar data/cotacoes.txt no formato que src/calculadora.rs lê.
7. Agenda: eventos de hoje na Início (bloco discreto, até 3) e uma tela de agenda com hoje e amanhã; sem configuração, não mostrar nada.
8. Backup: uma vez por dia com Wi-Fi, empacotar `<home>/data` (tar.gz, sem dicionários nem caches) e enviar; em Ajustes, "Backup: último ontem 21:04" e botão "Fazer backup agora"; "Restaurar" lista os backups e baixa/extrai com confirmação (mantendo cópia local do data atual antes).

Ideias novas escolhidas pelo Pedro:
9. Torneio dos treinadores: liga semanal com 5 treinadores sorteados perto do nível do Pedro, tabela de pontos, partidas pelo modo treinador existente.
10. Aberturas de xadrez: 10 aberturas clássicas; o treinador responde só a teoria da linha; a pessoa aprende jogando os lances da linha; progresso por abertura.
11. Comentário pós-partida + rival: no fim da partida contra treinador, os 3 momentos decisivos (maiores perdas registradas em coach Records) com tabuleiro e uma fala do treinador no jeito dele (personalidades em assets/treinadores/personalidades.txt, `crate::coach::persona`); um rival fixo (o treinador contra quem mais jogou) lembra o placar entre vocês e pede revanche na Início.
12. Histórias com dados: no formato de história (src/historias.rs), escolha com teste de sorte `* [dado 4] Pular o muro -> sucesso | falha` (rola 1d6, sucesso se >= N), com animação simples do dado; documente no cabeçalho do arquivo e use em pelo menos 3 pontos de uma história nova curta de exemplo; o verificador de loops (teste historias_sem_loop_e_com_finais) deve continuar valendo.
13. Carta do dia: de manhã, a Início mostra "Carta do dia" que abre um texto curto do Verso com o dia, agenda, clima e uma provocação para pensar (gerada uma vez por dia e guardada).
14. Rotina manhã/noite: sequência guiada de 5 a 10 minutos (passos configuráveis: respirar com contagem, planejar 3 coisas do dia, ler uma página, gratidão), em Mais, com sequência de dias.
15. Debate com o Verso: escolher um tema (ou sorteado), o Verso defende uma posição, a pessoa contra-argumenta pelo teclado por 3 rodadas, e no fim ele dá nota e comenta os argumentos.
16. Desafio do dia unificado: na Início, um quebra-cabeça por dia alternando xadrez (puzzle do dia), Sudoku, Coroas e Palavra, com uma sequência única de dias.
17. Resumo da semana em papel: aos domingos (ou em Mais › Semana), tela tipo jornal com o que leu, jogou, aprendeu, hábitos e a sequência.
18. Busca global: puxar para baixo na Início (ou botão) abre uma busca pelo teclado que acha livros, destaques, jogos, notas do diário, conversas do Verso e ajustes; tocar abre o item.

Ao terminar tudo, acrescente ao NOTAS-CODEX.md o resumo por item (o que fez, onde, testes, prints conferidos, limitações).
