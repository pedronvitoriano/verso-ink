# Verso Ink

Sistema próprio para o Kindle Paperwhite 3 (7ª geração, firmware 5.16.2.1.1), escrito em Rust.
O Kindle liga direto no Verso Ink: tela inicial com relógio e módulos (Ler, Jogar, Foco, Claude e Mais),
desenhada direto no e-ink, sem a interface da Amazon por cima.

> Versão 0.4.0. Tudo abaixo roda e foi testado num PW3 com firmware 5.16.2.1.1
> (relatório em `docs/testes/2026-11-11.md`).

## O que tem

| Bloco | O que faz |
|---|---|
| **Início** | Relógio grande, data, bateria, clima do dia (Open-Meteo) e um resumo: próximo lembrete, sequências, leitura e focos. **Continuar**: o livro em leitura com a % e o jogo em andamento. Tocar na data ou deslizar do topo abre **Luz e tela**. |
| **Ler** | Seus EPUB, PDF, AZW3, MOBI, CBZ e TXT com capa (extraída do livro, ou tipográfica) e o progresso do KOReader. Ordem por lidos por último ou A–Z, e busca no título. Abre no KOReader e volta sozinho. **Destaques**: o "My Clippings" agrupado por livro. |
| **Foco → Hoje** | Prioridades, tarefas, hábitos e lembretes do dia, marcados por toque, com sequência de dias. Itens podem valer só em alguns dias (`{seg,qua}`, `uteis`, `fds`). **Calendário** dos últimos 30 dias. **+ Adicionar** pelo teclado na tela. Pomodoro 25/5. Lembretes abrem em tela cheia na hora. |
| **Jogar** | 15 jogos sem Wi-Fi: Sudoku, xadrez (treinadores com Stockfish, revisão de partidas, duas pessoas), Coroas (fases surpresa até 12×12, sequência de dias), Lig 4, Pontos e caixas, Batalha naval, Campo minado, Nonograma, UNO, Palavra do dia, Sokoban, Paciência, Masmorra, Cripta e **Histórias** (ficção interativa por escolhas, em texto simples em `data/historias`). |
| **Mais** | Receitas com ingredientes para marcar e preparo passo a passo. |
| **Claude** | As sessões do Claude Code do seu PC, ao vivo: o que estão fazendo e quando é a sua vez. |
| **Luz e tela** | Luz 0–25 e modo noturno. **Ajustes**: Wi-Fi, endereço, SSH, bateria, versão, cópia de segurança, reiniciar, desligar e **Procurar atualização**. Uma cópia diária dos dados fica em `backups/` (as 7 mais novas) e uma versão nova aparece avisada no rodapé. |
| **Repouso** | Tela clara com o livro em andamento; Wi-Fi desliga ao dormir. Aviso de bateria fraca. |

## Como funciona

- **Um binário estático** (`armv7-unknown-linux-musleabi`, ~1,9 MB com as fontes): não depende da
  glibc do Kindle. Compilado com o `rust-lld` que vem com o Rust; nenhum compilador C é necessário.
- **Tela**: escreve em `/dev/fb0` e pede a atualização do e-ink pelo ioctl `MXCFB_SEND_UPDATE`
  (struct de 72 bytes do Kindle "Wario"). Toque responde com a forma de onda rápida (DU) e o conteúdo
  é desenhado com 16 cinzas (GC16).
- **Texto**: família [Inter](https://rsms.me/inter/) (SIL OFL), embutida, renderizada com `fontdue`.
- **Toque**: lê `/dev/input/event1` (cyttsp4, multitoque tipo B) com captura exclusiva.
- **Energia**: escuta o `powerd` (`lipc-wait-event`); ao dormir mostra a tela de repouso e desliga
  o Wi-Fi; ao acordar religa o Wi-Fi e acerta o relógio (`ntpdate`).
- **Sistema da Amazon**: a interface (Java) é parada enquanto o Verso Ink está aberto, o que libera
  RAM (de ~260 MB livres para ~430 MB). Kernel, drivers, Wi-Fi e energia continuam os da Amazon.

## Boot direto e saídas de emergência

Um job do upstart (`kindle/upstart/verso-ink.conf`) abre o Verso Ink quando a interface da Amazon
termina de subir. O Kindle volta para a interface da Amazon se:

1. existir o arquivo `/mnt/us/verso-ink/DISABLE` (pelo USB ou SSH);
2. houver **3 boots seguidos** sem o Verso Ink ficar 20 s de pé;
3. alguém **segurar o canto superior esquerdo** da tela durante "Abrindo…".

Se o Verso Ink cair a qualquer momento, a interface da Amazon volta sozinha.

## Requisitos

- Kindle desbloqueado (testado com WinterBreak) e acesso SSH (o do KOReader serve para a primeira vez).
- Rust com o alvo: `rustup target add armv7-unknown-linux-musleabi`.
- Chave SSH em `~/.ssh/kindle_ed25519` (a pública vai em `/mnt/us/verso-ink/settings/SSH/authorized_keys`).

## Uso

```bash
tools/deploy.sh 192.168.0.21          # compila e envia por Wi-Fi
ssh -p 2223 root@KINDLE /mnt/us/verso-ink/bin/ctl.sh open      # abre agora
ssh -p 2223 root@KINDLE /mnt/us/verso-ink/bin/ctl.sh boot-on   # liga direto no Verso Ink
ssh -p 2223 root@KINDLE /mnt/us/verso-ink/bin/ctl.sh boot-off  # volta a ligar na Amazon
tools/shot.sh tela.png                # captura a tela pela rede
```

O Verso Ink sobe seu próprio SSH na porta **2223** (só chave) antes de qualquer outra coisa, inclusive
quando o boot é pulado. Pelo KUAL há o menu **Verso Ink** com as mesmas opções.

### Pelo PC

```bash
tools/send.sh livro.epub                       # manda livros (aparecem no topo do Ler)
echo "texto" | tools/send.sh --texto "Título"  # manda um texto para ler
tools/dia.sh                                   # edita o dia (prioridades, tarefas, lembretes, hábitos)
tools/release.sh                               # publica uma versão (o Kindle se atualiza em Ajustes)
```

Formato do dia (`/mnt/us/verso-ink/data/dia.txt`): `! prioridade`, `- tarefa`, `@ 14:30 lembrete`, `* hábito`.

### Janela do Claude

Registre `tools/claude-hook.sh` como hook do Claude Code (eventos `SessionStart`, `UserPromptSubmit`,
`PreToolUse`, `Notification`, `Stop`, `SessionEnd`). Ele envia o estado ao Kindle em segundo plano e
sai em ~30 ms, então não atrasa o Claude Code; com o Kindle desligado, o envio só falha em silêncio.

### Ajustes em arquivo (`/mnt/us/verso-ink/settings/`)

- `utc_offset_min`: fuso em minutos (padrão −240, Manaus).
- `local`: `latitude longitude Nome` para o clima (padrão Manaus).
- `meta_leitura_min`: minutos por dia que contam para a sequência de leitura (padrão 20).

## Licença

AGPL-3.0. A fonte Inter é distribuída sob a SIL Open Font License (`assets/fonts/OFL-Inter.txt`).
