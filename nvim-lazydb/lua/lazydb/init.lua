local config = require("lazydb.config")

local M = {}

local state = {
  buf = nil,
  win = nil,
  augroup = nil,
}

local function buf_alive()
  return state.buf ~= nil and vim.api.nvim_buf_is_valid(state.buf)
end

local function is_visible()
  return state.win ~= nil and vim.api.nvim_win_is_valid(state.win)
end

local function create_float(buf)
  local opts = config.options
  local scale = opts.floating_window_scaling_factor
  local columns = vim.o.columns
  local lines = vim.o.lines

  local width = math.floor(columns * scale)
  local height = math.floor(lines * scale)
  local col = math.floor((columns - width) / 2)
  local row = math.floor((lines - height) / 2)

  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = width,
    height = height,
    col = col,
    row = row,
    style = "minimal",
    border = opts.floating_window_border_chars,
  })

  vim.api.nvim_set_option_value("winblend", opts.floating_window_winblend, { win = win })

  return win
end

local function setup_autocommands(buf)
  local ag = vim.api.nvim_create_augroup("LazyDB", { clear = true })
  state.augroup = ag

  vim.api.nvim_create_autocmd({ "BufEnter", "WinEnter" }, {
    group = ag,
    buffer = buf,
    callback = function()
      if buf_alive() then
        vim.cmd("startinsert")
      end
    end,
  })

  vim.api.nvim_create_autocmd({ "FocusGained", "TermLeave" }, {
    group = ag,
    callback = function()
      if is_visible() and vim.api.nvim_get_current_buf() == state.buf then
        vim.defer_fn(function()
          if is_visible() and vim.api.nvim_get_current_buf() == state.buf then
            vim.cmd("startinsert")
          end
        end, 0)
      end
    end,
  })
end

local function spawn()
  local buf = vim.api.nvim_create_buf(false, true)
  state.buf = buf

  state.win = create_float(buf)

  vim.fn.termopen(config.options.lazydb_command, {
    on_exit = function()
      vim.schedule(function()
        if state.augroup then
          vim.api.nvim_del_augroup_by_id(state.augroup)
          state.augroup = nil
        end
        if state.win and vim.api.nvim_win_is_valid(state.win) then
          vim.api.nvim_win_close(state.win, true)
        end
        if state.buf and vim.api.nvim_buf_is_valid(state.buf) then
          vim.api.nvim_buf_delete(state.buf, { force = true })
        end
        state.buf = nil
        state.win = nil
      end)
    end,
  })

  setup_autocommands(buf)
  vim.api.nvim_buf_set_keymap(buf, "t", "<Esc><Esc>", "<C-\\><C-n>:LazyDB<CR>", { noremap = true, silent = true })
  vim.cmd("startinsert")
end

function M.open()
  if is_visible() then
    vim.api.nvim_set_current_win(state.win)
    vim.cmd("startinsert")
    return
  end

  if buf_alive() then
    state.win = create_float(state.buf)
    vim.cmd("startinsert")
    return
  end

  spawn()
end

function M.hide()
  if not is_visible() then
    return
  end
  vim.api.nvim_win_close(state.win, true)
  state.win = nil
end

function M.kill()
  if buf_alive() then
    vim.api.nvim_buf_delete(state.buf, { force = true })
  end
  if state.win and vim.api.nvim_win_is_valid(state.win) then
    vim.api.nvim_win_close(state.win, true)
  end
  if state.augroup then
    vim.api.nvim_del_augroup_by_id(state.augroup)
    state.augroup = nil
  end
  state.buf = nil
  state.win = nil
end

function M.toggle()
  if is_visible() then
    M.hide()
  else
    M.open()
  end
end

function M.setup(opts)
  config.setup(opts)
end

return M
