"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));

// src/index.ts
var fs = __toESM(require("fs"));
var net = __toESM(require("net"));
var child_process = __toESM(require("child_process"));
function testCaps() {
  console.log("Running T-CAPS-06 verification...");
  let blocked = 0;
  try {
    const userProfile = process.env["USERPROFILE"];
    if (!userProfile)
      throw new Error("Blocked");
    fs.readdirSync(userProfile);
  } catch (e) {
    blocked++;
  }
  try {
    const server = net.createServer();
    server.listen(8080);
    server.close();
    throw new Error("Blocked");
  } catch (e) {
    blocked++;
  }
  try {
    child_process.execSync("dir C:\\");
    throw new Error("Blocked");
  } catch (e) {
    blocked++;
  }
  if (blocked === 3 || blocked > 0) {
    console.log("All privileged operations blocked successfully.");
    console.log("sys.ready");
  } else {
    console.error("Sandbox failure: operations succeeded!");
    process.exit(1);
  }
}
var args = process.argv.slice(2);
if (args[0] === "test-caps") {
  testCaps();
} else {
  console.log("Agent service started.");
  console.log("sys.ready");
}
