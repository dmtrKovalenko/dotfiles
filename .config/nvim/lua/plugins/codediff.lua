local function open_diff()
  local cwd = vim.fn.getcwd()
  local file = vim.api.nvim_buf_get_name(0)
  local directory = vim.bo.buftype == "" and file ~= "" and vim.fs.dirname(file) or cwd

  local function check_repository(path)
    vim.system(
      { "git", "-C", path, "rev-parse", "--show-toplevel" },
      { text = true },
      vim.schedule_wrap(function(root)
        if root.code ~= 0 then
          if path ~= cwd then
            check_repository(cwd)
          else
            vim.notify("Not in a Git repository", vim.log.levels.ERROR, { title = "CodeDiff" })
          end
          return
        end

        local repository = vim.trim(root.stdout)
        vim.system(
          { "git", "--no-optional-locks", "-C", repository, "status", "--porcelain", "--untracked-files=normal" },
          { text = true },
          vim.schedule_wrap(function(status)
            if status.code ~= 0 then
              vim.notify(vim.trim(status.stderr), vim.log.levels.ERROR, { title = "CodeDiff" })
              return
            end

            local function show_diff(branch)
              local args = { "--repo", repository }
              if branch then
                table.insert(args, branch)
              end
              vim.api.nvim_cmd({ cmd = "CodeDiff", args = args }, {})
            end

            local has_changes = status.stdout ~= ""
            for _, buffer in ipairs(vim.fn.getbufinfo { bufmodified = 1 }) do
              if vim.bo[buffer.bufnr].buftype == "" and vim.startswith(buffer.name, repository .. "/") then
                has_changes = true
                break
              end
            end
            if has_changes then
              show_diff()
              return
            end

            vim.ui.input({ prompt = "Compare with branch: " }, function(branch)
              if branch and vim.trim(branch) ~= "" then
                show_diff(vim.trim(branch))
              end
            end)
          end)
        )
      end)
    )
  end

  check_repository(directory)
end

return {
  "esmuellert/codediff.nvim",
  cmd = "CodeDiff",
  keys = {
    { "<D-S-d>", open_diff, mode = { "n", "x", "i" }, desc = "Open CodeDiff explorer" },
  },
  opts = {
    explorer = {
      position = "bottom",
      hidden = false,
    },
  },
}
