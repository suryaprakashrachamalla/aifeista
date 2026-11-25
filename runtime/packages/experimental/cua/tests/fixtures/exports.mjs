/** Built package smoke: headless consumers must not import the browser implementation. */
import assert from 'node:assert/strict'
import { registerHooks } from 'node:module'

const guard = registerHooks({
  resolve(specifier, context, next) {
    assert(!/^(?:react(?:-dom)?(?:\/|$)|@carnival\/dsh-client-)/.test(specifier), `Headless import reached ${specifier}`)
    const result = next(specifier, context)
    assert(!/\/cua\/lib\/(?:client(?:\/|\.)|types\/client\/)/.test(result.url), `Headless import reached ${result.url}`)
    return result
  },
})
const { Context } = await import('@carnival/cordis')
const host = await import('@carnival/dsh-cua')
const { createCuaApi } = await import('@carnival/dsh-cua/api')
const { CuaConfigFields } = await import('@carnival/dsh-cua/config')
assert.equal(typeof CuaConfigFields.connectTimeoutMs.default, 'function')
const { connectCua } = await import('@carnival/dsh-cua/bridge')
const { NativeClient } = await import('@carnival/dsh-cua/native-client')
const { Request } = await import('@carnival/dsh-cua/protocol')
for (const callable of [host.apply, host.installCua, host.CuaAccess, createCuaApi, connectCua, NativeClient]) assert.equal(typeof callable, 'function')
assert.equal(Request.parse({ method: 'getCapabilities', params: {} }).method, 'getCapabilities')
const schema = { type: 'object', properties: { argument: { type: 'null', description: 'Omit argument or pass null.' } }, required: [], additionalProperties: false }
const calls = []
const cua = createCuaApi(async operation => {
  calls.push(operation)
  if (operation.method === 'getCapabilities') return {
    platform: 'fixture', permissions: { accessibility: false, screenRecording: false }, appLevelAccessibility: false,
    keyboard: [], sessionLocked: true, methods: ['getActionSchema'], coordinateSpace: 'screenshot-pixels', browser: false,
  }
  if (operation.method === 'getApp') return { appHandle: 'app', name: 'Fixture', running: true }
  assert.equal(operation.method, 'getActionSchema')
  return schema
})
const app = await cua.getApp('Fixture')
assert.deepEqual(await app.getActionSchema('AXPress'), schema)
const query = { method: 'getActionSchema', params: { action: 'AXPress' } }
assert.deepEqual(calls.at(-1), query)
assert.deepEqual(Request.parse(query), query)
const ctx = new Context()
try { await ctx.plugin(host); assert.equal(ctx.get('cuaAccess'), undefined) }
finally { await ctx.fiber.dispose() }
guard.deregister()
const remote = await import('@carnival/dsh-cua/remote')
assert.equal(remote.default.package, '@carnival/dsh-cua')
assert.equal(remote.default.descriptors.length, 9)
process.stdout.write('headless exports and generated Remote passed\n')
