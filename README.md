# Code Usage

[![CI](https://github.com/puppe1990/code-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/puppe1990/code-usage/actions/workflows/ci.yml)

App de barra de menu (macOS) que mostra o usage do **Command Code**, **Grok** e **OpenCode** em um só lugar.

- **Título na barra:** `46% · $0.42 · $1.03` → percentual semanal do Grok · custo de hoje do Command Code · custo de hoje do OpenCode
- **Clique no ícone:** painel com hoje / 7 dias / 30 dias por provider, tokens (input, output, cache), o período semanal do Grok e o bloco de limites do Command Code (plano, % usado, requests, renovação e janelas de 5h/semanal)
- **Local por padrão:** custos e tokens vêm só dos arquivos que cada CLI grava na máquina. A **única** chamada de rede é para ler os limites do plano do Command Code (ver abaixo).

## Fontes de dados

| Provider               | Fonte                                                                                                   | O que é lido                                                                                                                                                                                    |
| ---------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Command Code           | `~/.commandcode/projects/<slug>/<session>.jsonl`                                                        | linhas de mensagem do assistant com `usage.costUsd` e tokens                                                                                                                                    |
| Command Code (limites) | `api.commandcode.ai` — `/alpha/usage/summary`, `/alpha/billing/credits`, `/alpha/billing/subscriptions` | plano, % do plano usado, requests do período, saldo de créditos e janelas de 5h/semanal (mesmos endpoints que o `/usage` do CLI usa, autenticados com o `apiKey` do `~/.commandcode/auth.json`) |
| Grok                   | `~/.grok/logs/unified.jsonl`                                                                            | eventos `billing: fetched credits config` (percentual do período) e `shell.turn.inference_done` (tokens)                                                                                        |
| OpenCode               | `~/.local/share/opencode/opencode.db` (SQLite, somente leitura)                                         | tabela `message`, JSON com `cost` e `tokens`                                                                                                                                                    |

Caminhos podem ser sobrescritos por variáveis de ambiente: `CODE_USAGE_CC_ROOT`, `CODE_USAGE_GROK_LOG`, `CODE_USAGE_OPENCODE_DB`, `CODE_USAGE_CC_AUTH`, `CODE_USAGE_CC_API_BASE`.

## Limitações (por design dos CLIs)

- **Command Code (limites)** usa a API interna do CLI — é um endpoint `alpha`, sem contrato público, e pode mudar sem aviso. Os limites são buscados no máximo a cada 5 minutos; se a chamada falhar, o app mantém o último valor lido (ou simplesmente não mostra o bloco) e continua funcionando com os dados locais.
- **Grok** só atualiza o percentual quando o CLI roda; o painel mostra "atualizado há X" com base no último evento.
- **OpenCode** não calcula custo para todas as mensagens (aquelas sem `cost` entram com custo 0, mas os tokens contam).

## Comportamento

- O título do tray é atualizado a cada **60s** e o painel recebe o novo snapshot por evento.
- Clique no ícone abre/fecha o painel; ele é posicionado logo abaixo do ícone e fecha ao perder o foco (Esc também fecha).
- O ícone não tem menu nativo: no macOS um menu anexado ao status item abriria em qualquer clique e impediria o popover, então **Atualizar agora** e **sair** ficam no próprio painel.
- O snapshot inteiro é recalculado a cada ciclo: Command Code lê os transcripts (~90 arquivos), Grok lê o log do CLI e OpenCode roda um `SELECT` somente-leitura na tabela `message`.
- Se um CLI não estiver instalado, o card mostra "não encontrado" e o título do tray mostra `–` naquela posição.

## Desenvolvimento

```bash
npm install          # o projeto tem .npmrc com include=dev (seu npm global tem omit=dev)
npm run tauri dev    # app rodando na barra de menu (vite na porta 1421)
cargo test           # testes Rust (parsers, janelas de tempo, título do tray) — em src-tauri/
cargo test -- --ignored --nocapture   # smoke test contra os dados reais da máquina
npm test             # testes do frontend (Vitest)
```

A porta 1421 é usada no dev porque a 1420 está ocupada por outro projeto (video-editor). A janela precisa da capability `core:default` em `src-tauri/capabilities/default.json` para usar `listen`/`invoke`.

## Qualidade (prettier, testes e CI)

- `npm run format` formata tudo com prettier (`npm run format:check` só verifica).
- **pre-commit** em `.githooks/pre-commit`, ativado automaticamente pelo `npm install` (script `prepare` que aponta `core.hooksPath`): roda `prettier --check`, os testes do frontend e `cargo test --release`. Para pular numa emergência: `git commit --no-verify`.
- **CI** em `.github/workflows/ci.yml`: job de frontend no ubuntu (prettier, `tsc --noEmit`, vitest) e job de Rust no macOS (`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --release`).

Para regenerar os ícones (fonte desenhada em `scripts/generate-icons.mjs`):

```bash
node scripts/generate-icons.mjs
npm run tauri -- icon src-tauri/icons/source.png
```

## Build e instalação

```bash
npm run tauri build
cp -R "src-tauri/target/release/bundle/macos/Code Usage.app" /Applications/
```

O app roda sem ícone no Dock (`ActivationPolicy::Accessory`); para sair, use o menu do ícone → **Sair**. Para iniciar no login, adicione em Ajustes do Sistema → Geral → Itens de Login.

## Arquitetura

- `src-tauri/src/usage/` — parsers puros e testáveis (`commandcode.rs`, `grok.rs`, `opencode.rs`), janelas de tempo (`window.rs`) e formatação do título (`tray_title.rs`)
- `src-tauri/src/tray.rs` — ícone da barra, menu e posicionamento do popover
- `src-tauri/src/refresh.rs` — recálculo a cada 60s + evento para o painel
- `src/` — painel em TypeScript puro (Vite), com helpers de formatação cobertos por Vitest
