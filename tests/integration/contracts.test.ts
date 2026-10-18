import { readFileSync } from 'node:fs';
import { strict as assert } from 'node:assert';
import { createHash } from 'node:crypto';
import Ajv2020 from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';
import canonicalize from 'canonicalize';
const root = new URL('../../', import.meta.url);
const read=(p:string)=>JSON.parse(readFileSync(new URL(p,root),'utf8'));
const schema=read('schemas/contracts.schema.json');
const ajv=new (Ajv2020 as unknown as {new(options:object): import('ajv').default})({strict:false,validateFormats:true});
(addFormats as unknown as (a:unknown)=>void)(ajv);
ajv.addSchema(schema);
let checked=0;
for(const c of read('fixtures/contracts/cases.json')) {
 if(c.level!=='schema') continue;
 const check=ajv.compile({$ref:schema.$id+'#/$defs/'+c.type});
 assert.equal(check(c.value),c.valid,c.name); checked++;
}
for(const c of read('fixtures/contracts/hash-golden.json')) {
 const encode=canonicalize as unknown as (v:unknown)=>string;
 assert.equal(createHash('sha256').update(encode(c.value)).digest('hex'),c.sha256);
}
for(const c of read('fixtures/contracts/search-hash-golden.json')) {
 assert.equal(createHash('sha256').update(c.text, 'utf8').digest('hex'),c.sha256);
}
console.log(`TypeScript/Ajv schema fixtures passed: ${checked}; RFC8785 vectors passed`);
