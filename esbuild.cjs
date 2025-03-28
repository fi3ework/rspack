var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) =>
	function __require() {
		return (
			mod ||
				(0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod),
			mod.exports
		);
	};

// B.js
var require_B = __commonJS({
	"B.js"(exports) {
		exports.B1 = {};
		exports.B2 = {};
		var A = require_A();
		console.log("A", A);
		exports.B3 = A.A1;
	}
});

// A.js
var require_A = __commonJS({
	"A.js"(exports) {
		exports.A1 = {};
		exports.A2 = {};
		var B = require_B();
		console.log("B", B);
		exports.A3 = B.B1;
	}
});
module.exports = require_A();
