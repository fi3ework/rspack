/** @type {import("@rspack/core").Configuration} */
module.exports = {
	entry: {
		index: "./index.js",
	},
	module: {
		parser: {
			javascript: {
				requireAsExpression: false,
			},
		},
	},
	output: {
		filename: `[name].js`,
		module: true,
		libraryTarget: "modern-module",
		iife: false,
		chunkFormat: "module",
	},
	experiments: {
		outputModule: true,
	},
	optimization: {
		concatenateModules: true,
		minimize: false,
	},
	plugins: [
		function () {
			/**
			 * @param {import("@rspack/core").Compilation} compilation compilation
			 * @returns {void}
			 */
			const handler = (compilation) => {
				compilation.hooks.afterProcessAssets.tap("testcase", (assets) => {
					expect(assets["index.js"]._value).toMatchSnapshot();
				});
			};
			this.hooks.compilation.tap("testcase", handler);
		},
	],
};
