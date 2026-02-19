-- Local project config

-- Guard against multiple loads (important)
if vim.g.project_config_loaded then
	return
end
vim.g.project_config_loaded = true

vim.lsp.config("rust_analyzer", {
	settings = {
		["rust-analyzer"] = {
			cargo = {
				features = "all",
			},
			checkOnSave = true,
		},
	},
})

vim.lsp.enable("rust_analyzer")
