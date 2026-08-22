use std::fs;
use std::path::Path;

fn main() {
    let grammar_dir = Path::new("src/parser/grammar");
    
    // Apenas concatena se o diretório grammar/ existir. Isso nos previne de falhar se estivermos no meio da refatoração.
    if grammar_dir.exists() && grammar_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(grammar_dir)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        
        // Ordena os arquivos pelo nome, garantindo a ordem: 01_base, 02_tipos, etc.
        entries.sort_by_key(|dir| dir.path());

        let mut combined_content = String::new();
        combined_content.push_str("// =========================================================================\n");
        combined_content.push_str("// ARQUIVO GERADO AUTOMATICAMENTE. NÃO EDITE.\n");
        combined_content.push_str("// Edite os arquivos em src/parser/grammar/ e este arquivo será gerado via build.rs.\n");
        combined_content.push_str("// =========================================================================\n\n");

        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("lalrpop_part") {
                let content = fs::read_to_string(&path).unwrap();
                combined_content.push_str(&format!("// --- Inicio: {} ---\n", path.file_name().unwrap().to_string_lossy()));
                combined_content.push_str(&content);
                combined_content.push_str("\n\n");
            }
        }

        // Escreve o conteúdo final em src/parser.lalrpop
        let parser_file = Path::new("src/parser.lalrpop");
        fs::write(parser_file, combined_content).unwrap();
    }

    lalrpop::process_root().unwrap();
}
