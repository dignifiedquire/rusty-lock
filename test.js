'use strict'

const fs = require('fs')
const lock = require('./')

const lockfile = './repo.lock'

try {
  fs.unlinkSync(lockfile)
} catch (err) {}

console.log('first lock')
lock.lock(lockfile)

console.log('second lock')
lock.lock(lockfile)

console.log('unlocking')
console.log(lock.unlock(lockfile))

console.log('third lock')
lock.lock(lockfile)

console.log(fs.readdirSync('./'))
