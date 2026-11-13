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

## O que implementar: Ajustes do sistema completos (tela Settings em src/apps.rs `draw_settings` e handlers em src/app.rs; SysInfo)

Hoje Ajustes tem bateria, cópia, reiniciar. Reorganize em seções (uma tela de lista de seções e uma tela por seção), no estilo do app, e implemente (um commit por seção):
1. Armazenamento: espaço total/livre de /mnt/us (statvfs) e quanto ocupam livros (documents), dicionários (dict), dados do Verso (data, separando partidas de xadrez, caderno, histórias geradas, backups locais), caches (cache, sprites redimensionados se houver) e logs; barra por categoria; ações com confirmação: limpar caches, apagar partidas de xadrez antigas (mais de N), apagar histórias geradas, apagar backups locais antigos, e uma lista dos 20 maiores livros com apagar (confirmação dupla, nunca apagar .sdr de outro livro). No PC use o diretório home do app.
2. Tela e energia: tempo para dormir (settings/repouso_min já existe: 5/10/15/30/60 min/nunca), tela de descanso (porta-retrato, capa do livro atual ou ilustração), luz automática por horário (ex.: 22h–7h luz baixa), modo economia (Wi-Fi desliga após N min sem uso).
3. Rede e conta: redes Wi-Fi salvas com "esquecer" (veja src/wifi.rs e o que o Kindle expõe por lipc), endereço da ponte da VPS e "testar conexão" (chama /health e mostra Claude/Codex/imagens prontos), modo avião agendado.
4. Sistema: versão (env!("VERSO_GIT")) e data da compilação, verificar atualização (src/update.rs), reiniciar o Verso Ink, reiniciar o Kindle (confirmação), restaurar padrões (só ajustes, nunca dados, confirmação), teclado (layout pt-BR já existe; opção de vibração não existe), ver registro de erros (últimas 50 linhas de /tmp/verso-ink.log com PANIC/erro), sobre (licenças: Inter OFL, Stockfish GPL, Dicionário Aberto domínio público, FreeDict GPL).

Cada ação destrutiva com confirmação clara e testes da lógica (cálculo de tamanhos, seleção do que apagar, nunca fora do home). Prints de cada seção conferidos.
Commits: se falhar por permissão, anote no NOTAS-CODEX.md e siga até o fim.
