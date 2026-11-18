const {run} = require('../parser/runtime/tree-sitter/index.cjs');
const config = require('./grammar.json');
run(config, __dirname).catch(() => { process.exitCode = 2; });
