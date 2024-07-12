set dotenv-load

export EDITOR := 'vim'

default:
  just --list

fmt:
  cargo fmt
  prettier --write .

dev *args:
  bunx concurrently \
    --kill-others \
    --names 'server,client' \
    --prefix-colors 'green.bold,magenta.bold' \
    --prefix '[{name}] ' \
    --prefix-length 2 \
    --success first \
    --handle-input \
    --timestamp-format 'HH:mm:ss' \
    --color \
    -- \
    'just watch run -- {{args}} serve' \
    'bun run dev'

watch +COMMAND='test':
  cargo watch --clear --exec "{{COMMAND}}"
