# Verso Ink

Sistema próprio para o Kindle Paperwhite 3 (7ª geração, firmware 5.16.2.1.1), escrito em Rust.
O Kindle liga direto no Verso Ink: tela inicial com relógio e módulos (Ler, Jogar, Foco, Verso e Mais),
desenhada direto no e-ink, sem a interface da Amazon por cima.

> Versão 0.4.0. Tudo abaixo roda e foi testado num PW3 com firmware 5.16.2.1.1
> (relatório em `docs/testes/2026-11-11.md`).

## O que tem

| Bloco | O que faz |
|---|---|
| **Início** | Relógio grande, data, bateria, clima do dia (Open-Meteo) e um resumo: próximo lembrete, sequências, leitura e focos. **Continuar**: o livro em leitura com a % e o jogo em andamento. Tocar na data ou deslizar do topo abre **Luz e tela**. |
| **Ler** | Seus EPUB, PDF, AZW3, MOBI, CBZ e TXT com capa (extraída do livro, ou tipográfica) e o progresso do KOReader. Ordem por lidos por último ou A–Z, e busca no título. Abre no KOReader e volta sozinho. Os KFX/AZW da Amazon também aparecem: o Verso Ink explica o limite do formato protegido e abre o Kindle original. **Destaques**: junta o `My Clippings.txt` e os destaques (`drawer`) dos sidecars do KOReader ao lado do livro; avisa quando o arquivo do Kindle está sem novidades há muito tempo. |
| **Foco → Hoje** | Prioridades, tarefas, hábitos e lembretes do dia, marcados por toque, com sequência de dias. Itens podem valer só em alguns dias (`{seg,qua}`, `uteis`, `fds`). **Calendário** dos últimos 30 dias. **+ Adicionar** pelo teclado na tela. Pomodoro 25/5. Lembretes abrem em tela cheia na hora. |
| **Jogar** | 17 jogos sem Wi-Fi: Sudoku, xadrez (treinadores com Stockfish, coordenadas legíveis, classificação e revisão dos lances), Coroas (fases surpresa até 12×12, sequência de dias), Lig 4, Pontos e caixas, Batalha naval, Campo minado, Nonograma, UNO, Palavra do dia, Sokoban, Paciência, Masmorra, Cripta, **Histórias** (ficção interativa por escolhas, em texto simples em `data/historias`), Damas e Velha. |
| **Mais** | Receitas com ingredientes para marcar e preparo passo a passo. |
| **Verso** | Conversa, arquivos e contexto no Kindle. O histórico é da conversa do Verso: Claude e Codex podem continuar um ao outro quando houver limite ou indisponibilidade temporária. Há controles visíveis para incluir o resumo seguro do Brain e criar uma imagem. |
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

### Assistente Verso: Claude, Codex, Brain e imagens

O Kindle chama o assistente de **Verso**. Por baixo, a ponte da VPS mantém o histórico canônico de cada
conversa e escolhe um motor: começa pelo último que respondeu (Claude ou Codex) e troca para o outro apenas
em caso de limite de uso, timeout, indisponibilidade temporária ou falta de login. Um pedido recusado ou
inválido não é repetido no outro motor. A resposta mostra, em texto pequeno, qual motor a escreveu e se houve
troca.

- **Brain**: o botão da pergunta envia somente o resumo seguro previamente sincronizado pelo PC com
  `tools/brain-resumo.sh`: contagens de retomadas e adequações. Ele não envia o Brain inteiro, títulos de
  notas pessoais, arquivos privados nem segredos. O botão pode ficar desligado em cada pergunta.
- **Arquivos no chat**: livros, destaques, cifras e fotos continuam sendo anexos da conversa. Arquivos criados
  pelos motores e imagens geradas aparecem em **Arquivos**; imagens abrem em tela cheia no próprio Verso Ink.
- **Imagem**: o botão **Imagem** usa a API de imagens da OpenAI, com uma chave exclusiva da VPS. Ele cria um
  PNG a partir da descrição; para não misturar referências ambíguas, anexos ficam desativados nesse modo.

### Ponte Verso na VPS

Instale ou atualize a ponte na VPS como `root`:

```bash
server/install-vps.sh
```

O script mantém o usuário restrito `versoink`, instala Claude Code e, se houver Node.js/npm disponível,
instala o Codex CLI em `/home/versoink/.local/bin/codex`. Depois, faça os logins **na VPS**, sem copiar as
credenciais do computador:

```bash
su - versoink -c /home/versoink/.local/bin/claude
# no Claude, use /login se ele pedir
su - versoink -c '/home/versoink/.local/bin/codex login --device-auth'
```

Para imagens, guarde uma chave de API da OpenAI somente no secret manager da VPS ou no arquivo
`/etc/verso-ink/openai-image-api-key`, lido pelo grupo `versoink` (`root:versoink`, modo `640`). Nunca coloque
essa chave em `settings/`, Git, Brain ou no chat. O endpoint `/health` informa se Claude, Codex e imagens estão
prontos, sem revelar credenciais.

A ponte também oferece backup dos dados do Kindle (`/backup`, até 50 MB, com as 14 cópias mais recentes),
agenda de hoje e amanhã (`/agenda`) e cotações (`/cotacoes`). Para ativar a agenda, salve a URL iCal secreta
em `/etc/verso-ink/agenda-ical-url` com dono `root:versoink` e modo `640`; ela não é devolvida ao Kindle.

Os nomes locais antigos `claude_url`, `claude_token` e `claude_resolve` continuam por compatibilidade: apontam
para a ponte do **Verso**, não definem qual motor será usado.

### Agentes no PC

Registre `tools/claude-hook.sh` como hook do Claude Code (eventos `SessionStart`, `UserPromptSubmit`,
`PreToolUse`, `Notification`, `Stop`, `SessionEnd`). Ele envia o estado ao Kindle em segundo plano e
sai em ~30 ms, então não atrasa o Claude Code; com o Kindle desligado, o envio só falha em silêncio. Essa tela
ainda acompanha somente o Claude Code no PC; o Codex já participa do chat na VPS, mas ainda não tem coletor de
sessão ao vivo.

### Ajustes em arquivo (`/mnt/us/verso-ink/settings/`)

- `utc_offset_min`: fuso em minutos (padrão −240, Manaus).
- `local`: `latitude longitude Nome` para o clima (padrão Manaus).
- `meta_leitura_min`: minutos por dia que contam para a sequência de leitura (padrão 20).

## Licença

AGPL-3.0. A fonte Inter é distribuída sob a SIL Open Font License (`assets/fonts/OFL-Inter.txt`).
