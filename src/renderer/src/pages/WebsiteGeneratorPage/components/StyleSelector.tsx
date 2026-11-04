import React from 'react'
import { Dropdown, DropdownItem } from 'flowbite-react'
import { Style } from '../templates'

interface StyleSelectorProps {
  currentStyle: Style
  styles: Style[]
  onSelect: (style: Style) => void
}

export const StyleSelector: React.FC<StyleSelectorProps> = ({ currentStyle, styles, onSelect }) => {
  return (
    <Dropdown label={currentStyle.label} dismissOnClick={true} size="xs" color={'indigo'}>
      {styles?.map((style) => (
        <DropdownItem key={style.value} onClick={() => onSelect(style)}>
          {style.label}
        </DropdownItem>
      ))}
    </Dropdown>
  )
}
