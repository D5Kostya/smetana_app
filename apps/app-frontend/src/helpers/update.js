import { ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { getArtifact, getOS } from '@/helpers/utils.js'

export const allowState = ref(false)
export const installState = ref(false)
export const updateState = ref(false)

const os = ref('')
const releaseLink = `https://api.github.com/repos/TeamYukizome/Modrinth-App-without-Ads/releases/latest`
const failedFetch = `Failed to fetch remote releases:`
const osNames = ['macos', 'windows', 'linux']
const fileExtensions = {
  macos: '.dmg',
  windows: '.msi',
  linux: '.AppImage' // Добавлено расширение для Linux
}
const blacklistedBuilds = [
  'dev',
  'nightly',
  'dirty',
  'dirty-dev',
  'dirty-nightly',
  'dirty_dev',
  'dirty_nightly',
]

export async function getRemote(elementIdBool, downloadArtifactBool) {
  try {
    const response = await fetch(releaseLink)
    if (!response.ok) throw new Error(`HTTP ${response.status}`)

    const data = await response.json()
    os.value = await getOS()
    const currentOS = os.value.toLowerCase()
    const remoteVersion = data.tag_name || data.name

    // Обновление элемента DOM если нужно
    if (!elementIdBool) {
      const releaseData = document.getElementById('releaseData')
      if (releaseData) releaseData.textContent = remoteVersion
    }

    // Проверка обновления
    if (osNames.includes(currentOS)) {
      const currentVersion = await getVersion()
      updateState.value = remoteVersion !== `v${currentVersion}` &&
        remoteVersion !== currentVersion
      allowState.value = updateState.value
    } else {
      updateState.value = false
      allowState.value = false
    }

    console.log('Update available:', updateState.value)
    console.log('Remote version:', remoteVersion)
    console.log('Local version:', await getVersion())
    console.log('OS:', os.value)

    // Скачивание артефакта если требуется
    if (downloadArtifactBool && allowState.value) {
      installState.value = true
      const fileName = getInstaller(fileExtensions[currentOS], data.assets)
      if (fileName) {
        await getArtifact(fileName.url, fileName.name, os.value, true)
      }
      installState.value = false
    }
  } catch (error) {
    console.error(failedFetch, error)
    if (!elementIdBool) {
      const errorData = document.getElementById('releaseData')
      if (errorData) errorData.textContent = error.message
    }
    updateState.value = false
    allowState.value = false
    installState.value = false
  }
}

function getInstaller(osExtension, assets) {
  if (!osExtension) return null

  for (const asset of assets) {
    const isValid = asset.name.endsWith(osExtension) &&
      !blacklistedBuilds.some(build => asset.name.startsWith(build))

    if (isValid) {
      return {
        name: asset.name,
        url: asset.browser_download_url
      }
    }
  }
  return null
}