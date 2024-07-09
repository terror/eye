set dotenv-load

export EDITOR := 'vim'

default:
  just --list

fmt:
  cargo fmt
