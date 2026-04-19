local config = require("lazydb.config")

local M = {}

local state = {
  buf = nil,
  win = nil,
}

local function is_open()
  return state.win ~= nil and vim.api.nvim_win_is_valid(state.win)
end

local function create_float()
  local opts = config.options
  local scale = opts.floating_window_scaling_factor
  local columns = vim.o.columns
  local lines = vim.o.lines

  local width = math.floor(columns * scale)
  local height = math.floor(lines * scale)
  local col = math.floor((columns - width) / 2)
  local row = math.floor((lines - height) / 2)

  local buf = vim.api.nvim_create_buf(false, true)

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

  return buf, win
end

function M.open()
  if is_open() then
    vim.api.nvim_set_current_win(state.win)
    return
  end

  local buf, win = create_float()
  state.buf = buf
  state.win = win

  vim.fn.termopen(config.options.lazydb_command, {
    on_exit = function()
      if state.win and vim.api.nvim_win_is_valid(state.win) then
        vim.api.nvim_win_close(state.win, true)
      end
      if state.buf and vim.api.nvim_buf_is_valid(state.buf) then
        vim.api.nvim_buf_delete(state.buf, { force = true })
      end
      state.buf = nil
      state.win = nil
    end,
  })

  vim.cmd("startinsert")
end

function M.close()
  if not is_open() then
    return
  end
  vim.api.nvim_win_close(state.win, true)
  if state.buf and vim.api.nvim_buf_is_valid(state.buf) then
    vim.api.nvim_buf_delete(state.buf, { force = true })
  end
  state.buf = nil
  state.win = nil
end

function M.toggle()
  if is_open() then
    M.close()
  else
    M.open()
  end
end

function M.setup(opts)
  config.setup(opts)
end

return M
