// src/codegen/cil/compilador_il.rs

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Localiza o executável `ilasm` no sistema operacional como fallback rápido.
fn ilasm_executable() -> Option<PathBuf> {
    // 1. Variável de ambiente ILASM
    if let Some(path) = env::var_os("ILASM") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    // 2. Windows: .NET Framework padrão
    #[cfg(windows)]
    {
        let default_fw = PathBuf::from(r"C:\Windows\Microsoft.NET\Framework64\v4.0.30319\ilasm.exe");
        if default_fw.is_file() {
            return Some(default_fw);
        }
        let default_fw32 = PathBuf::from(r"C:\Windows\Microsoft.NET\Framework\v4.0.30319\ilasm.exe");
        if default_fw32.is_file() {
            return Some(default_fw32);
        }
    }

    // 3. Verifica no PATH
    if let Ok(output) = Command::new("where.exe").arg("ilasm").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = s.lines().next() {
                let p = PathBuf::from(first_line.trim());
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Compila o código intermediário CIL (.il) para executável de máquina release.
///
/// Utiliza a infraestrutura oficial do .NET SDK via `Microsoft.NET.Sdk.IL` (gerando
/// binário nativo com Native AOT / self-contained) e possui fallback automático para `ilasm`.
pub fn compilar_cil_para_executavel(
    il_path: &Path,
    nome_base: &str,
    output_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    let out_dir = output_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    fs::create_dir_all(&out_dir).map_err(|e| format!("Falha ao criar diretório de saída: {}", e))?;

    let mut exe_name = nome_base.to_string();
    #[cfg(windows)]
    if !exe_name.ends_with(".exe") {
        exe_name.push_str(".exe");
    }
    let final_exe_path = out_dir.join(&exe_name);

    // 1. Tenta compilar via dotnet CLI com Microsoft.NET.Sdk.IL (Native AOT)
    println!("  📦 Compilando CIL com .NET SDK para código de máquina...");
    let temp_build_dir = out_dir.join(format!(".il_build_{}", nome_base));
    let _ = fs::create_dir_all(&temp_build_dir);

    let ilproj_path = temp_build_dir.join(format!("{}.ilproj", nome_base));
    let ilproj_content = format!(
        r#"<Project Sdk="Microsoft.NET.Sdk.IL/8.0.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <PublishAot>true</PublishAot>
    <MicrosoftNetCoreIlasmPackageVersion>8.0.0</MicrosoftNetCoreIlasmPackageVersion>
  </PropertyGroup>
</Project>
"#
    );

    if fs::write(&ilproj_path, ilproj_content).is_ok() {
        // Copia o .il para a pasta de build temporária
        let target_il = temp_build_dir.join("Principal.il");
        if fs::copy(il_path, &target_il).is_ok() {
            let dist_dir = temp_build_dir.join("dist");
            let mut dotnet_cmd = Command::new("dotnet");

            #[cfg(windows)]
            let rid = "win-x64";
            #[cfg(target_os = "linux")]
            let rid = "linux-x64";
            #[cfg(target_os = "macos")]
            let rid = "osx-x64";

            dotnet_cmd
                .arg("publish")
                .arg(&ilproj_path)
                .arg("-c")
                .arg("Release")
                .arg("-r")
                .arg(rid)
                .arg("-o")
                .arg(&dist_dir);

            if let Ok(output) = dotnet_cmd.output() {
                if output.status.success() {
                    let generated_bin = dist_dir.join(&exe_name);
                    if generated_bin.is_file() {
                        let _ = fs::copy(&generated_bin, &final_exe_path);
                        // Limpa pasta temporária de build
                        let _ = fs::remove_dir_all(&temp_build_dir);
                        return Ok(final_exe_path);
                    }
                }
            }
        }
    }

    // 2. Fallback: compilação direta via ilasm
    if let Some(ilasm) = ilasm_executable() {
        println!("  ⚠️ Fallback: Montando executável diretamente com ilasm...");
        let mut ilasm_cmd = Command::new(ilasm);
        ilasm_cmd
            .arg("/nologo")
            .arg("/exe")
            .arg(format!("/output:{}", final_exe_path.display()))
            .arg(il_path);

        let output = ilasm_cmd
            .output()
            .map_err(|e| format!("Falha ao executar ilasm: {}", e))?;

        if output.status.success() {
            // Limpa pasta temporária se existir
            let _ = fs::remove_dir_all(&temp_build_dir);
            return Ok(final_exe_path);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "Falha ao compilar CIL com ilasm:\nSTDOUT:\n{}\nSTDERR:\n{}",
                stdout, stderr
            ));
        }
    }

    Err(
        "Nenhum compilador CIL (.NET SDK com 'dotnet' ou 'ilasm') foi encontrado no sistema."
            .to_string(),
    )
}

