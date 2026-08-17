# habitus

CLI de acompanhamento de hábitos, direto no terminal. Sem app, sem nuvem, sem notificação empurrada — só um `done` por dia e um streak que não perdoa se você falhar.

Escrita em Rust, com banco local em SQLite. Segundo projeto da leva "fora do universo de segurança", depois do `mnemo`, reaproveitando a mesma stack pra fixar os conceitos de ownership, `rusqlite` e `clap` num domínio diferente.

## Por que existe

Streak rígido é feature, não bug: a ideia é que quebrar um dia realmente custe caro, criando pressão real pra manter consistência — o mesmo princípio por trás de apps como Duolingo, só que sem gamificação forçada, sem anúncio, e com os dados 100% locais.

## Como funciona hoje (MVP)

- Criação de hábitos (`habitus habit <nome>`)
- Check-in diário (`habitus done <nome>`) — cada chamada registra um novo check-in (não é mais idempotente de propósito: hábitos com meta > 1 precisam de múltiplos check-ins no mesmo dia)
- Meta diária configurável (`habitus target <nome> <N>`, padrão 1): um dia só "conta" pro streak quando o número de check-ins naquele dia atinge a meta — útil pra hábitos tipo "beber água 3x ao dia"
- Cálculo de **streak rígido**: falhou um dia (ou não bateu a meta diária), o streak volta a zero a partir dali. O dia atual tem tolerância (se ainda não fez hoje mas fez ontem, o streak continua contando normalmente até o dia acabar)
- **Recorde histórico** (`habitus status`): mostra não só o streak atual, mas a maior sequência de dias consecutivos já alcançada, mesmo que já tenha quebrado há tempos
- Comando `list` — mostra todos os hábitos com streak atual
- Comando `status <nome> --days N` — mostra streak atual, recorde, e grade de quadrados coloridos (verde = meta batida, cinza = não, via códigos ANSI) dos últimos N dias, estilo GitHub contribution graph
- Comando `habitus week` — resumo compacto de todos os hábitos de uma vez, cada um com streak atual + grade dos últimos 7 dias numa linha só
- Comando `habitus export <nome> [--output arquivo.csv]` — exporta o histórico de check-ins por dia (contagem, meta, se bateu ou não) pra CSV
- Lembretes (`habitus remind <nome> HH:MM`, `habitus remind <nome> --clear`): define um horário de lembrete por hábito. O comando `habitus check-reminders` dispara notificação via `termux-notification` pros hábitos que baterem o horário (janela de 5 min) e ainda não foram feitos hoje — pensado pra rodar automaticamente via `cron` (`crond`/`cronie` do Termux), não manualmente
- Comando `habitus delete <nome>` — remove um hábito e todo o seu histórico, com confirmação interativa (s/N) antes de executar
- Comando `habitus undo <nome>` — desmarca o check-in mais recente de hoje (corrige um `done` feito por engano), sem afetar dias anteriores
- Mensagens de erro amigáveis: pedir um hábito que não existe mostra uma explicação clara e o comando exato pra criá-lo, em vez do erro cru do SQLite
- Testes unitários (dentro de `streak.rs` e `stats.rs`) e testes de integração (`tests/integration_test.rs`, rodando contra um banco SQLite em memória) cobrindo o fluxo completo: criar → múltiplos check-ins → undo → delete → streak → correlação
- Correlação entre hábitos (`habitus correlate`): mostra o quanto cada par de hábitos "anda junto", usando índice de Jaccard (dias em comum / dias totais entre os dois) — ajuda a responder perguntas tipo "nos dias que eu treino, também leio mais?"
- Modo TUI (`habitus tui`): visão geral navegável (setas ↑/↓) de todos os hábitos, com streak e recorde de cada um. Apertando Enter num hábito, mostra a grade completa de 28 dias em tela cheia (Esc/Backspace volta pra lista, q sai)
- Metas por **frequência semanal** (`habitus target-weekly <nome> <N>`, ou `--clear` pra voltar ao modo diário): em vez de exigir N check-ins no MESMO dia, exige N check-ins em QUALQUER dia da mesma semana (ex: "leitura" 3x por semana). Streak e recorde nesse modo contam semanas consecutivas, não dias — o comando `status` mostra qual unidade se aplica a cada hábito

## Stack

- **Rust** (edition 2021)
- `rusqlite` (SQLite embutido, feature `bundled`)
- `clap` (parsing de CLI via derive macros)
- `chrono` (datas e cálculo de streak)
- `anyhow` (tratamento de erros simplificado)
- `csv` (exportação de histórico)
- `ratatui` + `crossterm` (interface TUI)

Estrutura do crate: `src/lib.rs` expõe os módulos (`db`, `models`, `streak`, `stats`, `tui`) como biblioteca, e `src/main.rs` é só a camada de CLI por cima dela — isso é o que permite os testes em `tests/integration_test.rs` existirem (testes de integração só enxergam a API pública de uma lib, não de um binário puro).

Lembretes automáticos dependem do app **Termux:API** instalado (pra `termux-notification` funcionar) e de um scheduler rodando em background — o projeto foi testado com `cronie` + `termux-services` (`pkg install cronie termux-services`, `sv-enable crond`), com uma entrada de crontab chamando `habitus check-reminders` a cada 5 minutos.

Ambiente de desenvolvimento: Termux/Android, sem Docker.

## Uso

```bash
cargo build --release

./target/release/habitus habit "treinar"
./target/release/habitus done "treinar"
./target/release/habitus list
./target/release/habitus status "treinar" --days 14
./target/release/habitus undo "treinar"

./target/release/habitus target "beber_agua" 3
./target/release/habitus done "beber_agua"

./target/release/habitus week
./target/release/habitus export "treinar" --output treinar.csv

./target/release/habitus remind "treinar" 07:00
./target/release/habitus check-reminders

./target/release/habitus delete "treinar"

./target/release/habitus correlate
./target/release/habitus tui

./target/release/habitus target-weekly "leitura" 3
./target/release/habitus done "leitura"

# rodar os testes (unitários + integração)
cargo test
```

## Roadmap

### Concluído
- [x] Comando `habitus delete <nome>` — remover hábito e seus check-ins
- [x] Comando `habitus undo <nome>` — desmarcar o check-in de hoje (correção de engano)
- [x] Mensagens de erro amigáveis quando o hábito não existe (hoje estoura erro cru do SQLite)
- [x] Grade colorida via ANSI (verde para feito, cinza para não feito) em vez de ▓/░
- [x] Streak "recorde" (maior streak histórico, não só o atual) por hábito
- [x] Múltiplos check-ins por dia com contagem (ex: "beber água" 3x ao dia) em vez de boolean simples
- [x] Exportação de histórico pra CSV
- [x] Comando `habitus week` — resumo semanal de todos os hábitos de uma vez
- [x] Lembrete configurável (horário) integrado com `termux-notification`
- [x] Modo TUI com `ratatui` pra visão geral de todos os hábitos
- [x] Estatísticas de correlação entre hábitos (`habitus correlate`, índice de Jaccard)
- [x] Testes de integração cobrindo o fluxo completo (criar → marcar → consultar)
- [x] Metas customizadas por frequência (ex: "3x por semana" em vez de diário, via `habitus target-weekly`)

### Curto prazo
*(nenhum item pendente no momento)*

### Médio prazo
*(nenhum item pendente no momento)*

### Longo prazo
*(nenhum item pendente no momento — roadmap original 100% concluído)*

### Ideias futuras
*(itens novos, ainda sem prioridade definida)*

## Licença

Projeto pessoal de estudo, sem licença formal definida ainda.

