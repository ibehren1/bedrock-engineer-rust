import { AWS_REGIONS } from '@/types/aws-regions'

type Translate = (key: string) => string

/** Bedrock-capable regions, used for the failover picker. */
export const bedrockRegions = AWS_REGIONS.filter((region) => region.bedrockSupported)

/**
 * Region `<optgroup>`s for the region selects. Shared by the credentials section
 * and the failover picker, which is why it lives outside both.
 */
export const buildRegionGroups = (t: Translate) => [
  {
    label: t('Bedrock Supported Regions'),
    options: bedrockRegions.map((region) => ({
      value: region.id,
      label: `${region.name} (${region.id})`
    }))
  },
  {
    label: t('Other Regions'),
    options: AWS_REGIONS.filter((region) => !region.bedrockSupported).map((region) => ({
      value: region.id,
      label: `${region.name} (${region.id})`
    }))
  }
]
