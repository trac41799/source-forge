// src-tauri/src/arch_engine.rs
//
// Architecture Decision Engine: generates project scaffold based on
// the chosen stack preset. Uses embedded templates for each stack.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::stack_registry::StackPreset;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldReport {
    pub stack_id: String,
    pub stack_name: String,
    pub files_created: Vec<String>,
    pub next_steps: Vec<String>,
}

pub fn scaffold_project(
    stack_id: &str,
    project_path: &str,
    project_name: &str,
) -> Result<ScaffoldReport, String> {
    let stack = StackPreset::get_by_id(stack_id)
        .ok_or_else(|| format!("Unknown stack: {}", stack_id))?;

    let base = Path::new(project_path);
    std::fs::create_dir_all(base).map_err(|e| format!("Cannot create project dir: {e}"))?;

    let mut report = ScaffoldReport {
        stack_id: stack.id.clone(),
        stack_name: stack.name.clone(),
        files_created: Vec::new(),
        next_steps: Vec::new(),
    };

    match stack.id.as_str() {
        "nextjs-supabase-vercel" | "nextjs-prisma-vercel" | "nextjs-sqlite-vercel" => {
            scaffold_nextjs(base, project_name, stack, &mut report)?;
        }
        "express-react-supabase" => {
            scaffold_express_react(base, project_name, &mut report)?;
        }
        "nextjs-supabase-fastapi" => {
            scaffold_nextjs(base, project_name, stack, &mut report)?;
            scaffold_fastapi_backend(base, &mut report)?;
        }
        _ => return Err(format!("No scaffold for stack: {}", stack.id)),
    }

    report.next_steps.push("1. Run: npm install".into());
    report.next_steps.push("2. Configure .env with DATABASE_URL".into());
    report.next_steps.push("3. Run: npm run dev".into());

    Ok(report)
}

fn w(base: &Path, rel: &str, content: &str, r: &mut ScaffoldReport) -> Result<(), String> {
    let path = base.join(rel);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("mkdir: {e}"))?;
    }
    if !path.exists() {
        std::fs::write(&path, content).map_err(|e| format!("write {}: {e}", rel))?;
        r.files_created.push(rel.to_string());
    }
    Ok(())
}

fn scaffold_nextjs(base: &Path, name: &str, stack: &StackPreset, r: &mut ScaffoldReport) -> Result<(), String> {
    let sqlite = stack.database == "sqlite";
    let mut dependencies = serde_json::Map::new();
    for (dep, version) in [
        ("next", "^14.2.0"),
        ("react", "^18.3.0"),
        ("react-dom", "^18.3.0"),
        ("@prisma/client", "^5.15.0"),
        ("zod", "^3.23.0"),
    ] {
        dependencies.insert(dep.to_string(), serde_json::Value::String(version.to_string()));
    }
    if !sqlite {
        dependencies.insert(
            "@supabase/supabase-js".to_string(),
            serde_json::Value::String("^2.43.0".to_string()),
        );
    }
    let pkg = serde_json::json!({
        "name": name, "version": "0.1.0", "private": true,
        "scripts": { "dev": "next dev", "build": "next build", "start": "next start", "lint": "next lint", "typecheck": "tsc --noEmit", "test": "node --test tests", "postinstall": "prisma generate" },
        "dependencies": dependencies,
        "devDependencies": { "typescript": "^5.5.0", "@types/node": "^20.14.0", "@types/react": "^18.3.0", "@types/react-dom": "^18.3.0", "prisma": "^5.15.0", "tailwindcss": "^3.4.0", "postcss": "^8.4.0", "autoprefixer": "^10.4.0" }
    });
    w(base, "package.json", &serde_json::to_string_pretty(&pkg).unwrap(), r)?;
    w(base, "next.config.js", "/** @type {import('next').NextConfig} */\nconst nextConfig = {};\nmodule.exports = nextConfig;\n", r)?;
    w(base, "tsconfig.json", "{\"compilerOptions\":{\"target\":\"ES2017\",\"lib\":[\"dom\",\"dom.iterable\",\"esnext\"],\"allowJs\":true,\"skipLibCheck\":true,\"strict\":true,\"noEmit\":true,\"esModuleInterop\":true,\"module\":\"esnext\",\"moduleResolution\":\"bundler\",\"resolveJsonModule\":true,\"isolatedModules\":true,\"jsx\":\"preserve\",\"incremental\":true,\"plugins\":[{\"name\":\"next\"}],\"paths\":{\"@/*\":[\"./*\"]}},\"include\":[\"next-env.d.ts\",\"**/*.ts\",\"**/*.tsx\",\".next/types/**/*.ts\"],\"exclude\":[\"node_modules\"]}", r)?;
    w(base, "tailwind.config.ts", "import type { Config } from 'tailwindcss';\nconst config: Config = { content: ['./app/**/*.{js,ts,jsx,tsx,mdx}'], theme: { extend: {} }, plugins: [] };\nexport default config;\n", r)?;
    w(base, "postcss.config.js", "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } };\n", r)?;

    let (prov, url_value) = if sqlite {
        ("sqlite", "\"file:./dev.db\"".to_string())
    } else {
        ("postgresql", "env(\"DATABASE_URL\")".to_string())
    };
    // A model is required: `prisma generate` exits 1 on a model-less schema
    // ("You don't have any models defined"), which broke `npm install`
    // (postinstall) and therefore the whole build for every scaffolded app.
    w(base, "prisma/schema.prisma", &format!("generator client {{\n  provider = \"prisma-client-js\"\n}}\n\ndatasource db {{\n  provider = \"{prov}\"\n  url      = {url_value}\n}}\n\nmodel User {{\n  id        String   @id @default(cuid())\n  email     String   @unique\n  name      String?\n  createdAt DateTime @default(now())\n}}\n"), r)?;

    w(base, "app/layout.tsx", &format!("import type {{ Metadata }} from 'next';\nimport './globals.css';\nexport const metadata: Metadata = {{ title: '{name}' }};\nexport default function RootLayout({{ children }}: {{ children: React.ReactNode }}) {{\n  return (<html lang=\"en\"><body>{{children}}</body></html>);\n}}\n"), r)?;
    w(base, "app/globals.css", "@tailwind base;\n@tailwind components;\n@tailwind utilities;\n", r)?;
    w(base, "app/page.tsx", &format!("export default function Home() {{\n  return (<main className=\"flex min-h-screen items-center justify-center\"><h1 className=\"text-4xl font-bold\">Welcome to {name}</h1></main>);\n}}\n"), r)?;
    w(base, "app/api/health/route.ts", "import { NextResponse } from 'next/server';\nexport async function GET() {\n  return NextResponse.json({ status: 'ok', timestamp: new Date().toISOString() });\n}\n", r)?;
    w(base, "lib/prisma.ts", "import { PrismaClient } from '@prisma/client';\nconst g = globalThis as unknown as { prisma: PrismaClient };\nexport const prisma = g.prisma || new PrismaClient();\nif (process.env.NODE_ENV !== 'production') g.prisma = prisma;\n", r)?;
    if !sqlite {
        w(base, "lib/supabase.ts", "import { createClient } from '@supabase/supabase-js';\nexport const supabase = createClient(process.env.NEXT_PUBLIC_SUPABASE_URL!, process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY!);\n", r)?;
    }
    let local_url = crate::database::local_database_url(&crate::database::local_db_password());
    if sqlite {
        w(base, ".env.example", "# App\nJWT_SECRET=\"change-me\"\nPORT=3000\nCLIENT_URL=\"http://localhost:3000\"\n# Database — SQLite file, zero setup (`npx prisma db push` creates it).\n# Migrate to Postgres later: change provider/url in prisma/schema.prisma and set DATABASE_URL.\nDATABASE_URL=\"file:./dev.db\"\n", r)?;
    } else {
        w(base, ".env.example", &format!("# App\nJWT_SECRET=\"change-me\"\nPORT=3000\nCLIENT_URL=\"http://localhost:3000\"\n# Database — local Docker Postgres by default (`docker compose up -d db`).\n# For hosted Postgres later (Supabase cloud, Neon, RDS), replace with its URL:\n# DATABASE_URL=\"postgresql://postgres:[PASSWORD]@db.[REF].supabase.co:5432/postgres\"\nDATABASE_URL=\"{local_url}\"\n# Supabase\nNEXT_PUBLIC_SUPABASE_URL=\"https://[REF].supabase.co\"\nNEXT_PUBLIC_SUPABASE_ANON_KEY=\"your-anon-key\"\n"), r)?;
    }
    if !sqlite {
        w(
        base,
        "docker-compose.yml",
        &crate::database::render_compose_yml(&crate::database::local_db_password()),
        r,
    )?;
    }
    w(base, ".gitignore", "node_modules/\n.next/\ndist/\n.env\n.env.local\n*.db\n.worktrees/\n", r)?;
    w(
        base,
        "tests/scaffold.test.mjs",
        "import test from 'node:test';\nimport assert from 'node:assert/strict';\nimport { existsSync, readFileSync } from 'node:fs';\nimport { fileURLToPath } from 'node:url';\n\nconst read = (rel) => readFileSync(fileURLToPath(new URL(rel, import.meta.url)), 'utf8');\n\ntest('scaffold defines a build script', () => {\n  const pkg = JSON.parse(read('../package.json'));\n  assert.ok(pkg.scripts.build, 'package.json must define a build script');\n});\n\ntest('scaffold ships the health route', () => {\n  assert.ok(existsSync(fileURLToPath(new URL('../app/api/health/route.ts', import.meta.url))));\n});\n",
        r,
    )?;

    r.next_steps.push("4. Run: npx prisma generate".into());
    r.next_steps.push("5. Set DATABASE_URL + Supabase keys in .env".into());
    Ok(())
}

fn scaffold_express_react(base: &Path, name: &str, r: &mut ScaffoldReport) -> Result<(), String> {
    let pkg = serde_json::json!({
        "name": name, "version": "0.1.0", "private": true,
        "scripts": { "dev": "vite", "build": "tsc && vite build", "dev:server": "tsx src/server.ts", "start": "node dist/server.js" },
        "dependencies": { "express": "^4.19.0", "cors": "^2.8.5", "@prisma/client": "^5.15.0", "@supabase/supabase-js": "^2.43.0", "zod": "^3.23.0" },
        "devDependencies": { "typescript": "^5.5.0", "vite": "^5.3.0", "@vitejs/plugin-react": "^4.3.0", "tsx": "^4.15.0", "prisma": "^5.15.0", "@types/express": "^4.17.0", "@types/cors": "^2.8.0" }
    });
    w(base, "package.json", &serde_json::to_string_pretty(&pkg).unwrap(), r)?;
    w(base, ".gitignore", "node_modules/\ndist/\n.env\n.worktrees/\n", r)?;
    let local_url = crate::database::local_database_url(&crate::database::local_db_password());
    w(base, ".env.example", &format!("DATABASE_URL=\"{local_url}\"\nPORT=3000\n# Hosted Postgres later: replace DATABASE_URL with the cloud URL.\n"), r)?;
    Ok(())
}

fn scaffold_fastapi_backend(base: &Path, r: &mut ScaffoldReport) -> Result<(), String> {
    let be = base.join("backend");
    std::fs::create_dir_all(&be).map_err(|e| format!("mkdir backend: {e}"))?;
    w(&be, "requirements.txt", "fastapi==0.111.0\nuvicorn==0.30.0\nsupabase==2.5.0\n", r)?;
    w(&be, "main.py", "from fastapi import FastAPI\nfrom fastapi.middleware.cors import CORSMiddleware\n\napp = FastAPI()\napp.add_middleware(CORSMiddleware, allow_origins=[\"*\"], allow_methods=[\"*\"], allow_headers=[\"*\"])\n\n@app.get(\"/api/health\")\nasync def health():\n    return {\"status\": \"ok\"}\n", r)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tp() -> (TempDir, String) {
        let d = TempDir::new().unwrap();
        let p = d.path().to_string_lossy().to_string();
        (d, p)
    }

    #[test]
    fn test_nextjs_creates_core_files() {
        let (_d, p) = tp();
        let r = scaffold_project("nextjs-supabase-vercel", &p, "na").unwrap();
        assert!(r.files_created.len() >= 8);
        assert!(Path::new(&p).join("package.json").exists());
        assert!(Path::new(&p).join("app/page.tsx").exists());
        assert!(Path::new(&p).join("app/api/health/route.ts").exists());
        assert!(Path::new(&p).join("prisma/schema.prisma").exists());
    }

    #[test]
    fn test_scaffold_output_is_buildable_typescript() {
        // Two defects found by the from-empty acceptance run, both of which made
        // every scaffolded Next.js app fail `tsc` and `next build`:
        //  - `{{`/`}}` written literally into plain (non-`format!`) strings;
        //  - a Prisma schema with no models, so `prisma generate` exits 1.
        let (_d, p) = tp();
        let report = scaffold_project("nextjs-prisma-vercel", &p, "na").unwrap();
        assert!(!report.files_created.is_empty());
        for rel in &report.files_created {
            if !rel.ends_with(".ts") && !rel.ends_with(".tsx") && !rel.ends_with(".mjs") {
                continue;
            }
            let content = std::fs::read_to_string(Path::new(&p).join(rel)).unwrap();
            assert!(!content.contains("{{"), "{rel} contains doubled braces");
            assert!(!content.contains("}}"), "{rel} contains doubled braces");
        }

        let schema =
            std::fs::read_to_string(Path::new(&p).join("prisma/schema.prisma")).unwrap();
        assert!(
            schema.contains("model "),
            "prisma generate needs at least one model"
        );

        let pkg = std::fs::read_to_string(Path::new(&p).join("package.json")).unwrap();
        for script in ["typecheck", "test", "postinstall"] {
            assert!(pkg.contains(&format!("\"{script}\"")), "package.json must define {script}");
        }

        // Local-first database: the scaffold ships a compose file for a pinned
        // Postgres and points DATABASE_URL at it (cloud stays a comment).
        let compose = std::fs::read_to_string(Path::new(&p).join("docker-compose.yml")).unwrap();
        assert!(compose.contains("postgres:16-alpine"), "pinned image");
        assert!(compose.contains("54322:5432"), "host port");
        assert!(compose.contains("pg_isready"), "healthcheck");
        let env_example = std::fs::read_to_string(Path::new(&p).join(".env.example")).unwrap();
        assert!(
            env_example.contains("DATABASE_URL=\"postgresql://postgres:postgres@localhost:54322/postgres\""),
            "local-first DATABASE_URL, got:\n{env_example}"
        );
        assert!(env_example.contains("# DATABASE_URL=\"postgresql://postgres:[PASSWORD]"), "cloud template kept as a comment");
    }

    #[test]
    fn test_sqlite_scaffold_needs_neither_docker_nor_cloud() {
        let (_d, p) = tp();
        let report = scaffold_project("nextjs-sqlite-vercel", &p, "na").unwrap();

        let schema =
            std::fs::read_to_string(Path::new(&p).join("prisma/schema.prisma")).unwrap();
        assert!(schema.contains("provider = \"sqlite\""), "sqlite provider");
        assert!(schema.contains("file:./dev.db"), "file URL");
        assert!(schema.contains("model "), "generate still needs a model");

        let pkg = std::fs::read_to_string(Path::new(&p).join("package.json")).unwrap();
        assert!(!pkg.contains("@supabase/supabase-js"), "no cloud client in a sqlite app");
        assert!(pkg.contains("@prisma/client"), "prisma stays for schema management");

        assert!(!Path::new(&p).join("lib/supabase.ts").exists());
        assert!(!Path::new(&p).join("docker-compose.yml").exists(), "no container to define");

        let env_example =
            std::fs::read_to_string(Path::new(&p).join(".env.example")).unwrap();
        assert!(env_example.contains("DATABASE_URL=\"file:./dev.db\""), "file URL default");
    }

    #[test]
    fn test_nextjs_prisma_uses_postgresql() {
        let (_d, p) = tp();
        scaffold_project("nextjs-supabase-vercel", &p, "na").unwrap();
        let s = std::fs::read_to_string(Path::new(&p).join("prisma/schema.prisma")).unwrap();
        assert!(s.contains("postgresql"));
    }

    #[test]
    fn test_express_creates_package_json() {
        let (_d, p) = tp();
        let r = scaffold_project("express-react-supabase", &p, "na").unwrap();
        assert!(Path::new(&p).join("package.json").exists());
        assert!(r.files_created.len() >= 2);
    }

    #[test]
    fn test_invalid_stack_errors() {
        let (_d, p) = tp();
        assert!(scaffold_project("nope", &p, "na").is_err());
    }
}
