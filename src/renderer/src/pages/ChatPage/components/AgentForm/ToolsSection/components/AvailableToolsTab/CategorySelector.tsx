import React from 'react'
import { useTranslation } from 'react-i18next'
import { CategorySelectorProps } from '../../types'

/**
 * ツールカテゴリ選択コンポーネント
 */
export const CategorySelector: React.FC<CategorySelectorProps> = ({
  selectedCategory,
  onChange
}) => {
  const { t } = useTranslation()

  return (
    <div className="flex items-center justify-end">
      <div className="flex items-center">
        <label className="mr-2 text-sm text-ink">{t('tools.category')}:</label>
        <select
          value={selectedCategory}
          onChange={onChange}
          className="text-sm border-strong rounded-control shadow-sm focus:ring-accent focus:border-accent bg-raised bg-surface text-ink"
        >
          <option value="general">{t('Tool Categories.General Purpose')}</option>
          <option value="coding">{t('Tool Categories.Software Development')}</option>
          <option value="design">{t('Tool Categories.Design & Creative')}</option>
          <option value="data">{t('Tool Categories.Data Analysis')}</option>
          <option value="business">{t('Tool Categories.Business & Productivity')}</option>
          <option value="custom">{t('Tool Categories.Custom Configuration')}</option>
          <option value="all">{t('Tool Categories.All Configuration')}</option>
        </select>
      </div>
    </div>
  )
}
