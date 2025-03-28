import { type LibConfig, defineConfig } from "@rslib/core";
import CircularDependencyPlugin from "circular-dependency-plugin";
import prebundleConfig from "./prebundle.config.mjs";

const externalFunction = ({ request }: { request?: string }, callback) => {
	const { dependencies } = prebundleConfig;

	for (const item of dependencies) {
		const depName = typeof item === "string" ? item : item.name;
		if (new RegExp(`^${depName}$`).test(request!)) {
			return callback(null, `../compiled/${depName}/index.js`);
		}
	}

	if (/..\/package\.json/.test(request!)) {
		return callback(null, "../package.json");
	}

	return callback();
};

const commonLibConfig: LibConfig = {
	dts: false,
	format: "cjs",
	syntax: ["node 16"],
	source: {
		define: {
			__webpack_require__: "__webpack_require__"
		}
	},
	output: {
		cleanDistPath: false,
		// distPath: {
		// 	root: "./dist-rslib"
		// },
		externals: [externalFunction]
	}
};

export default defineConfig({
	lib: [
		{
			...commonLibConfig,
			tools: {
				rspack: {
					output: {
						library: {
							type: "commonjs"
						}
					}
				}
			},
			source: {
				entry: {
					index: "./src/index.ts"
				}
			},
			output: {
				...commonLibConfig.output,
				externals: [externalFunction, "./moduleFederationDefaultRuntime.js"]
			}
		},
		{
			...commonLibConfig,
			source: {
				entry: {
					cssExtractLoader: "./src/builtin-plugin/css-extract/loader.ts"
				}
			}
		},
		{
			...commonLibConfig,
			syntax: "es2015",
			source: {
				entry: {
					cssExtractHmr: "./src/runtime/cssExtractHmr.ts"
				}
			}
		}
	],
	output: {
		target: "node"
	},
	tools: {
		rspack: {
			plugins: [
				new CircularDependencyPlugin({
					failOnError: false,
					exclude: /node_modules/
				})
			]
		}
	}
});
