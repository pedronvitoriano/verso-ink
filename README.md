# Verso Ink

Sistema próprio para o Kindle Paperwhite 3 (7ª geração, firmware 5.16.2.1.1), escrito em Rust.
O Kindle liga direto no Verso Ink: tela inicial com relógio e módulos (Ler, Jogar, Foco, Claude e Mais),
desenhada direto no e-ink, sem a interface da Amazon por cima.

> Estado: **fundação**. Tela, texto, toque, repouso e boot direto funcionam; os módulos ainda são
> telas de apresentação ("em construção").

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

Fuso horário: minutos em relação ao UTC em `/mnt/us/verso-ink/settings/utc_offset_min` (padrão −240).

## Licença

AGPL-3.0. A fonte Inter é distribuída sob a SIL Open Font License (`assets/fonts/OFL-Inter.txt`).
