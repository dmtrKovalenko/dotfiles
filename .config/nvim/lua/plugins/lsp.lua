return {
  {
    "stevearc/conform.nvim",
    config = function()
      require("conform").setup {
        formatters_by_ft = {
          lua = { "stylua" },
          rust = { "rustfmt" },
          c = { "clang-format" },
          cpp = { "clang-format" },
          python = { "isort", "black" },
          javascript = { "prettierd", "prettier", stop_after_first = true },
          markdown = { "prettierd", "prettier", stop_after_first = true },
          typescript = { "prettierd", "prettier", stop_after_first = true },
          typescriptreact = { "prettierd", "prettier", stop_after_first = true },
          css = { "prettierd", "prettier", stop_after_first = true },
          svg = { "xmlformat" },
          json = { "prettierd", "prettier", stop_after_first = true },
          yaml = { "prettierd", "prettier", stop_after_first = true },
          graphql = { "prettierd", "prettier", stop_after_first = true },
          rescript = { "rescript-format" },
          ocaml = { "ocamlformat" },
          sql = { "pg_format" },
          proto = { "clang-format" },
          ocaml_mlx = { "ocamlformat_mlx" },
        },
      }

      local function format()
        require("conform").format {
          lsp_fallback = true,
        }
      end

      vim.keymap.set({ "n", "i" }, "<F12>", format, { desc = "Format", silent = true })
      vim.keymap.set({ "n", "i" }, "<C-f>", format, { desc = "Format", silent = true })

      vim.api.nvim_create_user_command("Format", format, { desc = "Format current buffer with LSP" })
    end,
  },
  {
    -- LSP Configuration & Plugins
    "williamboman/mason.nvim",
    lazy = false,
    dependencies = {
      {
        "folke/lazydev.nvim",
        ft = "lua", -- only load on lua files
        opts = {
          library = {
            -- See the configuration section for more details
            -- Load luvit types when the `vim.uv` word is found
            { path = "${3rd}/luv/library", words = { "vim%.uv" } },
          },
        },
      },
      "ocaml-mlx/ocaml_mlx.nvim",
      {
        "pmizio/typescript-tools.nvim",
        dependencies = { "nvim-lua/plenary.nvim", "neovim/nvim-lspconfig" },
        opts = {},
      },
      {
        "mrcjkb/rustaceanvim",
        lazy = false,
      },
      {
        "aznhe21/actions-preview.nvim",
        event = "LspAttach",
        opts = {
          diff = {
            algorithm = "patience",
            ignore_whitespace = true,
          },
        },
      },
      {
        "luckasRanarison/tailwind-tools.nvim",
        name = "tailwind-tools",
        build = ":UpdateRemotePlugins",
        event = "BufWinEnter",
        dependencies = {
          "nvim-treesitter/nvim-treesitter",
        },
        opts = {
          custom_filetypes = "rescript",
        },
      },
      {
        "rachartier/tiny-code-action.nvim",
        dependencies = {
          { "nvim-lua/plenary.nvim" },
        },
        event = "LspAttach",
        opts = {
          backend = "delta",
          picker = {
            "snacks",
            opts = {
              layout = {
                preset = "dropdown",
              },
            },
          },
        },
      },
    },
    config = function()
      local on_lsp_attach = function(client, bufnr)
        local lsp_map = function(keys, func, desc)
          if desc then
            desc = "LSP: " .. desc
          end

          vim.keymap.set("n", keys, func, { remap = true, buffer = bufnr, desc = desc, silent = true })
        end

        lsp_map("<D-.>", require("tiny-code-action").code_action, "Code Action")
        lsp_map("<D-i>", function()
          if client.name == "rust-analyzer" then
            vim.cmd.RustLsp { "hover", "actions" }
          else
            vim.lsp.buf.hover()
          end
        end, "Hover Documentation")
        lsp_map("<D-r>", vim.lsp.buf.rename, "Rename")
        lsp_map("gD", vim.lsp.buf.definition, "Goto Declaration")
        lsp_map("gi", vim.lsp.buf.implementation, "Goto Implementation")
        lsp_map("<D-g>", "<C-]>", "[G]oto [D]efinition")
        lsp_map("<D-u>", vim.lsp.buf.signature_help, "Signature Documentation")

        -- Various picker for lsp related stuff
        lsp_map("gr", Snacks.picker.lsp_references, "[G]oto [R]eferences")
        lsp_map("gi", Snacks.picker.lsp_implementations, "[G]oto [I]mplementations")
        lsp_map("gt", Snacks.picker.lsp_type_definitions, "[G]oto [T]ype Definitions")
        lsp_map("<D-l>", Snacks.picker.lsp_workspace_symbols, "Search workspace symbols")
        lsp_map("<leader>ss", Snacks.picker.lsp_symbols, "[S]earch [S]ymbols")

        lsp_map("<leader>lr", function()
          vim.cmd "LspRestart"
        end, "Lsp [R]eload")
        lsp_map("<leader>li", function()
          vim.cmd "LspInfo"
        end, "Lsp [R]eload")
        lsp_map("<leader>lh", function()
          vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled(filter), { bufnr })
        end, "Lsp toggle inlay [h]ints")
      end

      -- vim.diagnostic.config { virtual_lines = true }
      vim.diagnostic.config { virtual_text = true }

      -- nvim-cmp supports additional completion capabilities, so broadcast that to servers
      local capabilities = vim.lsp.protocol.make_client_capabilities()
      capabilities = vim.tbl_deep_extend("force", capabilities, require("blink.cmp").get_lsp_capabilities({}, false))
      -- optimizes cpu usage source https://github.com/neovim/neovim/issues/23291
      capabilities.workspace.didChangeWatchedFiles.dynamicRegistration = false

      vim.diagnostic.config {
        signs = {
          text = {
            [vim.diagnostic.severity.ERROR] = "󰚌 ",
            [vim.diagnostic.severity.WARN] = " ",
            [vim.diagnostic.severity.INFO] = " ",
            [vim.diagnostic.severity.HINT] = "󱧡 ",
          },
          numhl = {
            [vim.diagnostic.severity.ERROR] = "DiagnosticSignError",
            [vim.diagnostic.severity.WARN] = "DiagnosticSignWarn",
            [vim.diagnostic.severity.INFO] = "DiagnosticSignInfo",
            [vim.diagnostic.severity.HINT] = "DiagnosticSignHint",
          },
          texthl = {
            [vim.diagnostic.severity.ERROR] = "DiagnosticSignError",
            [vim.diagnostic.severity.WARN] = "DiagnosticSignWarn",
            [vim.diagnostic.severity.INFO] = "DiagnosticSignInfo",
            [vim.diagnostic.severity.HINT] = "DiagnosticSignHint",
          },
        },
      }

      local border = {
        { "╭", "FloatBorder" },
        { "─", "FloatBorder" },
        { "╮", "FloatBorder" },
        { "│", "FloatBorder" },
        { "╯", "FloatBorder" },
        { "─", "FloatBorder" },
        { "╰", "FloatBorder" },
        { "│", "FloatBorder" },
      }

      local handlers = {
        ["textDocument/hover"] = vim.lsp.with(vim.lsp.handlers.hover, {
          border = border,
        }),
        ["textDocument/signatureHelp"] = vim.lsp.with(vim.lsp.handlers.signature_help, {
          border = border,
        }),
      }

      -- Your existing floating preview override
      local orig_util_open_floating_preview = vim.lsp.util.open_floating_preview
      function vim.lsp.util.open_floating_preview(contents, syntax, opts, ...)
        opts = opts or {}
        opts.border = opts.border or border
        return orig_util_open_floating_preview(contents, syntax, opts, ...)
      end

      -- Set up global defaults first
      vim.lsp.config("*", {
        capabilities = capabilities,
        on_attach = on_lsp_attach,
        handlers = handlers,
        -- root_markers = { '.git' },
      })

      -- Servers with custom command or settings need vim.lsp.config()
      vim.lsp.config("clangd", {
        cmd = {
          "clangd",
          "--offset-encoding=utf-16",
        },
      })

      vim.lsp.config("lua_ls", {
        settings = {
          Lua = {
            runtime = { version = "LuaJIT" },
            workspace = { checkThirdParty = false },
            telemetry = { enable = false },
          },
        },
      })

      vim.lsp.config("bashls", {
        settings = { includeAllWorkspaceSymbols = true },
      })

      vim.lsp.config("typos_lsp", {
        single_file_support = false,
        init_options = { diagnosticSeverity = "WARN" },
      })

      -- Simplified relay_lsp configuration for debugging
      -- First, let's disable the automatic enable to test manually
      -- vim.lsp.config("relay_lsp", {
      --   cmd = { "relay-compiler", "lsp" },
      --   root_markers = { "relay.config.*", "package.json" },
      --   filetypes = { "javascript", "javascriptreact", "typescript", "typescriptreact" },
      -- })

      -- Temporarily disabled to prevent crashes - use RelayLspDiag to analyze setup
      -- vim.lsp.enable "relay_lsp"

      -- Create user commands for Relay LSP management
      vim.api.nvim_create_user_command("RelayLspRestart", function()
        vim.cmd "LspRestart relay_lsp"
      end, { desc = "Restart Relay LSP server" })

      -- Manual relay_lsp test command
      vim.api.nvim_create_user_command("RelayLspManualTest", function()
        local cwd = vim.uv.cwd()
        local filepath = vim.api.nvim_buf_get_name(0)
        local file_dir = vim.fn.fnamemodify(filepath, ":h")

        -- Find root directory
        local function find_root_dir(start_path, patterns)
          local current_dir = start_path
          while current_dir and current_dir ~= "/" do
            for _, pattern in ipairs(patterns) do
              local files = vim.fn.glob(current_dir .. "/" .. pattern, false, true)
              if #files > 0 then
                return current_dir, files[1]
              end
            end
            current_dir = vim.fn.fnamemodify(current_dir, ":h")
          end
          return nil, nil
        end

        local root_patterns = { "relay.config.*", "package.json" }
        local found_root, root_file = find_root_dir(file_dir, root_patterns)

        if not found_root then
          vim.notify("❌ No root directory found (no relay.config.* or package.json)", vim.log.levels.ERROR)
          return
        end

        -- Check for relay-compiler
        local relay_paths = {
          found_root .. "/node_modules/.bin/relay-compiler",
          vim.fn.fnamemodify(found_root, ":h") .. "/node_modules/.bin/relay-compiler",
          "relay-compiler"
        }

        local found_relay = nil
        for _, path in ipairs(relay_paths) do
          if vim.fn.executable(path) == 1 then
            found_relay = path
            break
          end
        end

        if not found_relay then
          vim.notify("❌ relay-compiler not found in any expected locations", vim.log.levels.ERROR)
          return
        end

        vim.notify("✅ Found root: " .. found_root, vim.log.levels.INFO)
        vim.notify("✅ Found relay-compiler: " .. found_relay, vim.log.levels.INFO)

        -- Create a simple relay_lsp configuration
        vim.lsp.config("relay_lsp_manual", {
          cmd = { found_relay, "lsp" },
          root_markers = { "relay.config.*", "package.json" },
          filetypes = { "javascript", "javascriptreact", "typescript", "typescriptreact" },
        })

        -- Try to start it
        vim.cmd("LspStart relay_lsp_manual")
        vim.notify("🔄 Started relay_lsp_manual - check :LspInfo", vim.log.levels.INFO)
      end, { desc = "Manually test relay_lsp with current setup" })

      vim.api.nvim_create_user_command("RelayCompilerStart", function()
        local root_dir = vim.lsp.get_clients({ name = "relay_lsp" })[1]
        if root_dir then
          root_dir = root_dir.config.root_dir
        else
          root_dir = vim.uv.cwd()
        end

        local relay_compiler_path = root_dir .. "/node_modules/.bin/relay-compiler"
        if vim.fn.executable(relay_compiler_path) ~= 1 then
          relay_compiler_path = "relay-compiler"
        end

        local watch_cmd = { relay_compiler_path, "--watch" }
        vim.fn.jobstart(watch_cmd, {
          cwd = root_dir,
          detach = true,
          on_stdout = function(_, data)
            if data and #data > 0 then
              vim.notify("Relay Compiler: " .. table.concat(data, "\n"), vim.log.levels.INFO)
            end
          end,
          on_stderr = function(_, data)
            if data and #data > 0 then
              vim.notify("Relay Compiler Error: " .. table.concat(data, "\n"), vim.log.levels.WARN)
            end
          end,
        })
        vim.notify("Started Relay compiler in watch mode", vim.log.levels.INFO)
      end, { desc = "Start Relay compiler in watch mode" })

      -- Comprehensive relay_lsp diagnostic command
      vim.api.nvim_create_user_command("RelayLspDiag", function()
        local results = {}

        -- Current buffer info
        local bufnr = vim.api.nvim_get_current_buf()
        local filepath = vim.api.nvim_buf_get_name(bufnr)
        local filetype = vim.bo[bufnr].filetype

        table.insert(results, "=== RELAY LSP DIAGNOSTICS ===")
        table.insert(results, "Current file: " .. filepath)
        table.insert(results, "Current filetype: " .. filetype)
        table.insert(results, "")

        -- Check if filetype matches relay_lsp filetypes
        local relay_filetypes = { "javascript", "javascriptreact", "typescript", "typescriptreact" }
        local filetype_match = vim.tbl_contains(relay_filetypes, filetype)
        table.insert(results, "Filetype matches relay_lsp: " .. (filetype_match and "✅ YES" or "❌ NO"))
        table.insert(results, "Expected filetypes: " .. table.concat(relay_filetypes, ", "))
        table.insert(results, "")

        -- Root directory detection (mimicking LSP's upward search)
        local function find_root_dir(start_path, patterns)
          local current_dir = start_path
          while current_dir and current_dir ~= "/" do
            for _, pattern in ipairs(patterns) do
              local files = vim.fn.glob(current_dir .. "/" .. pattern, false, true)
              if #files > 0 then
                return current_dir, files[1]
              end
            end
            current_dir = vim.fn.fnamemodify(current_dir, ":h")
          end
          return nil, nil
        end

        local file_dir = vim.fn.fnamemodify(filepath, ":h")
        local root_patterns = { "relay.config.*", "package.json" }
        local found_root, root_file = find_root_dir(file_dir, root_patterns)

        table.insert(results, "Current file directory: " .. file_dir)
        table.insert(results, "Current working directory: " .. vim.uv.cwd())

        if not found_root then
          table.insert(results, "❌ No root markers found")
          table.insert(results, "Searched upward from: " .. file_dir)
          table.insert(results, "Looking for: " .. table.concat(root_patterns, ", "))
        else
          table.insert(results, "✅ Root directory: " .. found_root)
          table.insert(results, "Root marker found: " .. root_file)
          table.insert(results, "Distance from CWD: " .. (found_root == vim.uv.cwd() and "same" or "different"))
        end
        table.insert(results, "")

        -- Check relay-compiler installation (using detected root or fallbacks)
        local relay_paths = {}
        local found_relay = false

        if found_root then
          -- Primary: check in project's node_modules
          table.insert(relay_paths, found_root .. "/node_modules/.bin/relay-compiler")

          -- Secondary: check in parent directory (for monorepos)
          local parent_dir = vim.fn.fnamemodify(found_root, ":h")
          table.insert(relay_paths, parent_dir .. "/node_modules/.bin/relay-compiler")
        end

        -- Fallback: global installation
        table.insert(relay_paths, "relay-compiler")

        for i, path in ipairs(relay_paths) do
          local executable = vim.fn.executable(path)
          local status = executable == 1 and "✅ FOUND" or "❌ NOT FOUND"
          local description = ""
          if i == 1 then
            description = " (project node_modules)"
          elseif i == 2 then
            description = " (parent/monorepo node_modules)"
          elseif path == "relay-compiler" then
            description = " (global)"
          end

          table.insert(results, "relay-compiler path " .. i .. ": " .. path .. description .. " - " .. status)
          if executable == 1 and not found_relay then
            found_relay = path
          end
        end
        table.insert(results, "")

        -- Check LSP clients
        local clients = vim.lsp.get_clients({ bufnr = bufnr })
        table.insert(results, "LSP clients attached to buffer: " .. #clients)

        local relay_client = nil
        for _, client in ipairs(clients) do
          table.insert(results, "  - " .. client.name .. " (id: " .. client.id .. ")")
          if client.name == "relay_lsp" then
            relay_client = client
          end
        end

        if relay_client then
          table.insert(results, "✅ relay_lsp client found!")
          table.insert(results, "  Root dir: " .. (relay_client.config.root_dir or "unknown"))
          table.insert(results, "  Command: " .. (relay_client.config.cmd and table.concat(relay_client.config.cmd, " ") or "unknown"))
        else
          table.insert(results, "❌ relay_lsp client not attached")
        end
        table.insert(results, "")

        -- Check if relay_lsp config exists
        local has_config = pcall(function()
          return vim.lsp.config.get("relay_lsp")
        end)
        table.insert(results, "relay_lsp config loaded: " .. (has_config and "✅ YES" or "❌ NO"))
        table.insert(results, "")

        -- Manual attachment analysis (without actually starting LSP)
        table.insert(results, "=== ANALYSIS ===")
        if found_relay and filetype_match and found_root then
          table.insert(results, "✅ All prerequisites met for relay_lsp!")
          table.insert(results, "Expected root: " .. found_root)
          table.insert(results, "Expected compiler: " .. found_relay)
          table.insert(results, "")
          table.insert(results, "To manually test relay_lsp:")
          table.insert(results, "  1. Run :LspStart relay_lsp")
          table.insert(results, "  2. Check :LspInfo for relay_lsp")
          table.insert(results, "  3. Look for 'relay_lsp starting in:' notification")
        else
          table.insert(results, "❌ Prerequisites not met:")
          if not found_relay then table.insert(results, "  - relay-compiler not found in any checked paths") end
          if not filetype_match then table.insert(results, "  - filetype '" .. filetype .. "' doesn't match expected types") end
          if not found_root then table.insert(results, "  - no root directory found (no relay.config.* or package.json upward from file)") end
        end

        table.insert(results, "")
        table.insert(results, "=== TROUBLESHOOTING ===")
        table.insert(results, "If relay_lsp doesn't work:")
        table.insert(results, "  1. Make sure you have relay-compiler installed")
        table.insert(results, "  2. Check that relay.config.* exists")
        table.insert(results, "  3. Verify you're in a JavaScript/TypeScript file")
        table.insert(results, "  4. Run :LspInfo to see attached clients")

        -- Show results - ensure no multi-line strings
        local flat_results = {}
        for _, line in ipairs(results) do
          local str_line = tostring(line)
          if str_line:find("[\r\n]") then
            -- Split multi-line strings
            for split_line in str_line:gmatch("[^\r\n]+") do
              table.insert(flat_results, split_line)
            end
          else
            table.insert(flat_results, str_line)
          end
        end

        vim.cmd("new")
        vim.api.nvim_buf_set_lines(0, 0, -1, false, flat_results)
        vim.bo.buftype = "nofile"
        vim.bo.bufhidden = "wipe"
        vim.bo.filetype = "text"
      end, { desc = "Diagnose relay_lsp configuration and attachment" })

      -- Servers that work with defaults can use vim.lsp.enable()
      vim.lsp.enable "lua_ls"
      vim.lsp.enable "dhall_lsp_server"
      vim.lsp.enable "marksman"
      vim.lsp.enable "taplo"
      vim.lsp.enable "astro"
      vim.lsp.enable "eslint"
      vim.lsp.enable "html"
      vim.lsp.enable "pylsp"
      vim.lsp.enable "zls"
      vim.lsp.enable "ocamllsp"

      require("typescript-tools").setup {
        on_attach = on_lsp_attach,
        handlers = handlers,
      }

      vim.g.rustaceanvim = {
        -- LSP configuration
        server = {
          on_attach = on_lsp_attach,
          logfile = "/tmp/rustaceanvim.log",
          default_settings = {
            -- rust-analyzer language server configuration
            ["rust-analyzer"] = {
              check = {
                allTargets = false,
              },
              cargo = {
                targetDir = true,
              },
              files = {
                excludeDirs = { "target", "node_modules", ".git", ".sl" },
              },
            },
          },
        },
        dap = {},
      }

      require("mason").setup()
    end,
  },
}
