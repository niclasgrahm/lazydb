vim.api.nvim_create_user_command("LazyDB", function()
  require("lazydb").toggle()
end, {})

vim.api.nvim_create_user_command("LazyDBOpen", function()
  require("lazydb").open()
end, {})

vim.api.nvim_create_user_command("LazyDBHide", function()
  require("lazydb").hide()
end, {})

vim.api.nvim_create_user_command("LazyDBKill", function()
  require("lazydb").kill()
end, {})

vim.api.nvim_create_user_command("LazyDBWithQuery", function()
  local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
  local query = table.concat(lines, "\n")
  require("lazydb").open_with_query(query)
end, {})
