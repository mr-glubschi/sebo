// Berechnet den aktuellen TOTP-Code aus einer otpauth://-URL (nur für den Rauchtest).
import crypto from "node:crypto";

const url = new URL(process.argv[2]);
const secret = url.searchParams.get("secret").replace(/=+$/, "");
const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
let bits = "";
for (const c of secret) bits += alphabet.indexOf(c).toString(2).padStart(5, "0");
const key = Buffer.from(bits.match(/.{8}/g).map((b) => parseInt(b, 2)));
const step = Math.floor(Date.now() / 1000 / 30);
const message = Buffer.alloc(8);
message.writeBigUInt64BE(BigInt(step));
const hmac = crypto.createHmac("sha1", key).update(message).digest();
const offset = hmac[19] & 15;
console.log(String((hmac.readUInt32BE(offset) & 0x7fffffff) % 1e6).padStart(6, "0"));
