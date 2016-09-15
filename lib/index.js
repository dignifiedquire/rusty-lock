'use strict'

const path = require('path')
const locks = require('../native')

exports.lock = function lock (filename) {
  const abs = path.resolve(filename)

  return locks.tryLockExclusive(abs)
}

exports.unlock = function unlock (filename) {
  const abs = path.resolve(filename)

  return locks.unlock(abs)
}
