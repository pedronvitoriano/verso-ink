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

## O que fazer: refazer o Domínio (src/dominio.rs) no estilo State.io, em tempo real

O Domínio atual (grade 4×4 de quadrados, turnos) foi reprovado pelo Pedro no visual e na jogabilidade. Ele quer o estilo de State.io / Nations Royale: territórios num mapa que geram soldados sozinhos, você toca num território seu e depois num alvo, e uma coluna de soldados marcha até lá; ao chegar, cada soldado abate um do defensor; se zerar, o território vira seu. Neutros (cinza) não crescem ou crescem devagar. Inimigos (CPU, 1 a 3) fazem o mesmo.

Mapa e visual (e-ink 1072×1448, só tons de cinza, sem cor):
- Mapa de 10 a 24 territórios IRREGULARES: gere sementes com distância mínima e divida a área do mapa por Voronoi (cada pixel ao território da semente mais próxima, com fronteira preta de 3 a 4 px), recortado numa forma de ilha/continente orgânica (não um retângulo), com mar em textura clara (linhas onduladas finas). Pré-renderize o mapa base em memória uma vez por fase (é caro) e só redesenhe por cima o que muda.
- Dono por padrão de preenchimento, nunca por cor: você = cinza escuro liso; CPU 1 = hachura diagonal; CPU 2 = pontilhado; CPU 3 = xadrez fino; neutro = branco/cinza muito claro.
- No centro de cada território, um círculo branco com borda preta grossa e o número de soldados em fonte Display grande e legível; o seu círculo com borda dupla. Território selecionado: anel grosso pulsando (alternando a cada tick).
- Soldados em marcha: pontinhos (ou pequenas figuras) andando em fila na linha entre os centros, com o número restante.

Jogabilidade:
- Tempo real usando `fn tick(&mut self) -> Redraw` do trait MiniGame (o app chama periodicamente; veja src/app.rs `tick`): a cada ~1 s os territórios crescem (+1 seu/inimigo até um teto por tamanho; neutros não), e as colunas avançam. Use Redraw::Part com a área do mapa para o e-ink atualizar rápido; pause quando o jogo não estiver em primeiro plano (keep_awake/resume como os outros).
- Toque: 1º toque num território seu seleciona; 2º toque num alvo (vizinho ou não) envia metade dos soldados (arredondado para baixo, mínimo 1); tocar no mesmo cancela; arrastar do seu para o alvo também envia (veja drag/drag_end). Enviar para território seu reforça.
- CPU: a cada poucos ticks, ataca o alvo mais fraco ao alcance quando tem vantagem, reforça fronteiras ameaçadas; dificuldade por fase (velocidade de decisão e agressividade).
- Fases: 30 mapas gerados por semente (fase 1: 1 CPU e poucos territórios; crescendo até 3 CPUs e 24 territórios), vitória quando só você resta; derrota se perder todos. Fase do dia e sequência como os outros jogos com fases (src/fases.rs). Pausa e "Recomeçar fase". Velocidade ajustável (Lento/Normal) porque o e-ink pisca.
- Teste: simulação headless de 200 partidas CPU×CPU por fase-amostra termina (sem travar) e o mapa Voronoi cobre tudo sem território vazio; regras de chegada (abate, conquista, reforço).
- Como jogar (src/tutorial.rs) atualizado; ícone continua icone-dominio.png.

Prints: menu, início da fase, seleção, soldados em marcha, conquista, vitória. Confira cada print: precisa parecer um mapa de verdade com territórios orgânicos, bonito e legível no e-ink.

Commits: se falhar por permissão, anote no NOTAS-CODEX.md e termine o trabalho mesmo assim.
