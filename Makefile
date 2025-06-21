.PHONY: test test-unit test-integration test-targets clean

test: test-unit test-integration test-targets

test-unit:
	@echo "🧪 Executando testes unitários..."
	cargo test --lib

test-integration:
	@echo "🔗 Executando testes de integração..."
	cargo test --test integration_tests

test-targets:
	@echo "🎯 Testando todos os targets..."
	./testar_targets.sh

clean:
	@echo "🧹 Limpando arquivos gerados..."
	cargo clean
	rm -f *.ll *.il *.js *.bytecode
	rm -rf *_Console/
	rm -rf tests/test_files/*.ll tests/test_files/*.il tests/test_files/*.js

build-release:
	@echo "🚀 Build de release..."
	cargo build --release

install:
	@echo "📦 Instalando compilador..."
	cargo install --path .
