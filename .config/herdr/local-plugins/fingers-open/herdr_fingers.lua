local M = { version = "0.1.1" }
local uv = vim.uv or vim.loop
local registration
local enabled = false

local function state_dir()
  return vim.env.HERDR_FINGERS_OPEN_STATE
    or ((vim.env.XDG_CACHE_HOME or (vim.env.HOME .. "/.cache")) .. "/herdr-fingers-open")
end

local function details()
  local files = {}
  for _, buffer in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(buffer) and vim.bo[buffer].buftype == "" then
      local name = vim.api.nvim_buf_get_name(buffer)
      if name ~= "" then
        table.insert(files, uv.fs_realpath(name) or name)
      end
    end
  end
  return {
    ok = true,
    version = M.version,
    pid = vim.fn.getpid(),
    pane_id = vim.env.HERDR_PANE_ID,
    herdr_socket = vim.env.HERDR_SOCKET_PATH,
    socket = vim.v.servername,
    nvim_bin = vim.v.progpath,
    cwd = vim.fn.getcwd(),
    current_file = vim.api.nvim_buf_get_name(0),
    buffers = files,
  }
end

function M.register()
  if not vim.env.HERDR_PANE_ID or not vim.env.HERDR_SOCKET_PATH then
    return false
  end
  if vim.v.servername == "" then
    vim.fn.serverstart()
  end
  local directory = state_dir() .. "/nvim"
  vim.fn.mkdir(directory, "p", 448)
  local key = vim.fn.sha256(vim.env.HERDR_SOCKET_PATH .. ":" .. vim.fn.getpid())
  registration = directory .. "/" .. key .. ".json"
  local temporary = registration .. ".tmp"
  vim.fn.writefile({ vim.json.encode(details()) }, temporary)
  uv.fs_chmod(temporary, 384)
  assert(uv.fs_rename(temporary, registration))
  return true
end

function M.status()
  return vim.json.encode(details())
end

function M.open(reference)
  local ok, error = pcall(function()
    assert(type(reference) == "table" and type(reference.path) == "string", "Missing file path")
    local stat = uv.fs_stat(reference.path)
    assert(stat and (stat.type == "file" or stat.type == "directory"), "Path no longer exists: " .. reference.path)
    if stat.type == "directory" then
      local loaded, oil = pcall(require, "oil")
      assert(loaded, "A directory was selected, but oil.nvim is not available in this editor.")
      if vim.bo.modified then
        vim.cmd("tabnew")
      end
      oil.open(reference.path)
      M.register()
      return
    end
    -- Reuse an existing window for this file when possible. Otherwise keep
    -- modified buffers loaded while displaying the requested file.
    local buffer = vim.fn.bufadd(reference.path)
    vim.fn.bufload(buffer)
    local windows = vim.fn.win_findbuf(buffer)
    if #windows > 0 then
      vim.api.nvim_set_current_win(windows[1])
    else
      if vim.bo.modified and vim.bo.bufhidden ~= "" and vim.bo.bufhidden ~= "hide" then
        vim.cmd("tabnew")
      end
      local hidden = vim.o.hidden
      vim.o.hidden = true
      local changed, reason = pcall(vim.api.nvim_win_set_buf, 0, buffer)
      vim.o.hidden = hidden
      assert(changed, reason)
    end
    local row = math.max(1, math.min(tonumber(reference.line) or 1, vim.api.nvim_buf_line_count(buffer)))
    local line = vim.api.nvim_buf_get_lines(buffer, row - 1, row, false)[1] or ""
    local column = math.max(0, math.min((tonumber(reference.column) or 1) - 1, #line))
    vim.api.nvim_win_set_cursor(0, { row, column })
    vim.cmd("normal! zvzz")
    M.register()
  end)
  return vim.json.encode({ ok = ok, error = not ok and tostring(error) or nil })
end

function M.setup()
  if enabled then
    M.register()
    return
  end
  enabled = true
  local group = vim.api.nvim_create_augroup("HerdrFingers", { clear = true })
  vim.api.nvim_create_user_command("HerdrFingersRegister", function()
    if not M.register() then
      vim.notify("This Neovim is not running in a Herdr pane.", vim.log.levels.WARN)
    end
  end, { force = true })
  vim.api.nvim_create_autocmd({ "VimEnter", "DirChanged", "BufEnter", "FocusGained" }, {
    group = group,
    callback = function()
      vim.schedule(function()
        pcall(M.register)
      end)
    end,
  })
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = group,
    callback = function()
      if registration then
        uv.fs_unlink(registration)
      end
    end,
  })
  M.register()
end

return M
