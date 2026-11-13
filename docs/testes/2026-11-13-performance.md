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
