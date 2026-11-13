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
