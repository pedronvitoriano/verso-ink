# Performance no Kindle — linha de base (13/11/2026, antes da etapa de performance)

Kindle Paperwhite 3, `verso-ink bench` (desenho em memória, sem e-ink). Binário de b2ef15b+0c62075.

```
destaques (contagem)    188.2 ms
ofensiva (status)         1.5 ms
  · encher a tela                1.593 ms
  · cartão 928×110               6.498 ms
  · disco arredondado 80         4.047 ms
  · texto 30px (40 letras)       8.129 ms
  · texto 130px (título)        10.669 ms
  · medir texto 30px             0.076 ms
  · traço (stroke) 100px         7.782 ms
  · ilustração 340              10.610 ms
  · ícone em disco 104          13.219 ms
Início                 desenho  305.0 ms · toque   1.5 ms
Jogar                  desenho  312.4 ms · toque   0.0 ms
Ler                    desenho  156.2 ms · toque   0.0 ms
Hoje                   desenho  151.3 ms · toque   0.0 ms
Calendário             desenho   96.9 ms · toque   0.0 ms
Mais                   desenho  207.9 ms · toque   0.0 ms
Luz e tela             desenho  285.2 ms · toque   0.0 ms
Edição                 desenho  320.0 ms · toque   0.0 ms
Achados                desenho  104.5 ms · toque   0.0 ms
Aprender               desenho  218.7 ms
```

## Depois (13/11/2026)

| Medida no Kindle | Antes | Cache de glifos | + ponto flutuante em hardware (musleabihf) |
|---|---|---|---|
| Primeira tela no boot | 1.087 ms | 1.087 ms | **305 ms** |
| Início | 305 ms | 201 ms | **85 ms** |
| Jogar | 312 ms | 167 ms | **67 ms** |
| Edição | 320 ms | 205 ms | **98 ms** |
| Mais | 208 ms | 124 ms | **55 ms** |
| Aprender | 219 ms | 144 ms | **64 ms** |
| Texto 40 letras | 8,1 ms | 0,48 ms | 0,32 ms |
| Contagem de destaques | 188 ms | 1,5 ms | 1,5 ms |
| Cartão / traço / ícone | 6,5 / 7,8 / 13 ms | igual | 1,5 / 0,9 / 2,7 ms |

A troca da divisão por deslocamento (06dcbfe) não mudou nada: o compilador já otimizava. Desenho idêntico pixel a pixel (testes `texto_com_cache_igual_ao_antigo` e `mix_igual_a_divisao_para_todos_os_valores`).

## Jogos (13/11/2026, Kindle, p50/p95 de 15 desenhos)

| Jogo | Antes | Depois | O que mudou |
|---|---|---|---|
| Coroas: criar fase 30 | 9.004 ms | 2 ms | banco de 505 tabuleiros feito no PC |
| Lig 4: desenho | 120 ms | 16 ms | buraco vazio desenhado uma vez e copiado |
| UNO: desenho | 61 ms | 23 ms | oval da carta por faixas (era 1 fill por pixel) |
| Todas as telas | opt-level "s" | opt-level 3 | 4 a 10% mais rápido |

Ainda os mais lentos: Coroas 70 ms, Masmorra 56 ms (p95 126), Sokoban 41 ms (p95 82). Perfil por primitiva: `VERSO_INK_BENCH_JOGO=Nome verso-ink bench`.

## Todos os jogos, do toque à tela pronta (13/11/2026, noite)

Método: `verso-ink bench-jogos 200` no Kindle Paperwhite 3, com `VERSO_INK_HOME` numa cópia de
`data`, `settings` e `cache` **sem** `settings/claude_*` (o comando se recusa a rodar com a ponte
configurada). Cada um dos 22 jogos da tela Jogar (Sudoku e Xadrez inclusive) é aberto e jogado
pelo próprio app com 200 toques em alvos sorteados (sempre a mesma sequência): menus, partida,
fim, tutorial, jogada da CPU. Mede do toque até a tela pronta na memória (sem o e-ink e sem a
pausa proposital do "Pensando…"). CPU ociosa antes de medir (load 0,02). `VERSO_INK_BENCH_PERFIL=1`
imprime o perfil por primitiva de cada toque acima de 150 ms.

Desenho (bench antigo, p50/p95 de 15 desenhos): Masmorra 67/103 → 41/60 ms; Sokoban 45/91 → 29/69 ms;
Coroas 23/33 → 24/33; os outros iguais ou melhores. Desenho idêntico ponto a ponto (testes
`rapido_igual_ao_antigo`, `cartao_rapido_igual_ao_antigo`, `reducao_rapida_igual_a_antiga`).

| Toque | Antes | Depois | O que mudou |
|---|---|---|---|
| Coroas: prática 12×12 | 5.525 ms | instantâneo | sorteada do banco feito no PC (10×10 e 12×12) |
| Tutorial (Ajuda, Próxima, Anterior), todos os jogos | 210–275 ms | 140–155 ms | miniatura reduzida uma vez (a 1ª era coberta) e redução sem divisões por ponto |
| Lig 4: pior jogada da CPU | 1.093 ms | 695 ms | poda na raiz e prazo de 650 ms (fica com a última busca completa) |
| Super Velha e Naval: espera da CPU | pausa + conta (até ~2,5 s) | a maior das duas | a conta corre dentro do "Pensando…" (como o treinador do xadrez) |
| Xadrez: abrir a revisão de partida antiga | 154 ms + gravação a cada lance rejogado | 154 ms | rejogar não grava mais (gravava por cima da partida em andamento) |
| Histórias (no 1º passeio) | 41.655 ms | — | era uma chamada real ao Claude; a medição agora roda sem a ponte |

Depois, p50/p95/pior por jogo (ms): Sudoku 22/23/57 · Xadrez 49/69/154 · Coroas 44/53/125 ·
Lig 4 40/86/695 · Caixas 19/60/148 · Naval 741* · Minas 21/44/87 · Nonograma 18/25/87 ·
UNO 51/98/164 · Palavra 25/27/96 · Sokoban 31/32/153 · Paciência 24/47/155 · Masmorra 46/63/119 ·
Cripta 28/29/90 · Histórias 4/46/92 · Damas 0/42/88 · Velha 50/61/98 · Reversi 90/180/241* ·
Go 136/253/360* · Cruzadas 4/42/51 · Domínio 43/45/80 · Detetive 38/43/106.

\* Naval, Reversi e Go: o tempo é a jogada da CPU (no Naval, 0,7 s de pausa para mostrar a água).
Ela corre dentro da pausa do "Pensando…" (0,45 a 1,6 s), então não aumenta a espera.

**Limitações e o que fica:**
- Continuar uma partida contra treinador liga o Stockfish e analisa a posição por 600 ms depois
  de desenhar: a tela aparece na hora, mas o primeiro toque espera até ~1 s. Mudar exige o motor
  rodando em paralelo (mexe em toda a lógica do treinador).
- O passeio sorteia toques: telas raras podem não ser visitadas em 200 toques (a prática 12×12 do
  Coroas foi visitada 1 vez). Os testes de tempo diretos cobrem as operações corrigidas.
- Tempo de tela pronta na memória; o e-ink soma de ~120 ms (rápido) a ~450 ms (com flash).

**Regressão:** `tempo_dos_jogos::todos_os_jogos_respondem_rapido` (passeio no PC, p50 até 10 ms e
pior até 150 ms; Naval até 900) e testes de tempo diretos com tetos que reprovam a versão antiga
(medido no PC, antes → depois): prática 12×12 144 → 0,03 ms (teto 20); CPU do Lig 4, profundidade
8, 23,6 → 6,5 ms (teto 15); página de tutorial com cena 12,9 → 4,3 ms (teto 9). Menor de várias
medidas, para não falhar com a máquina ocupada.
