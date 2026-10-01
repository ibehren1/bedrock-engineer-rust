import React from 'react'

export const LoadingSkeleton: React.FC = () => {
  return (
    <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5">
      {[...Array(8)].map((_, index) => (
        <div
          key={index}
          className="bg-surface rounded-container shadow-sm border border-subtle p-5 animate-pulse"
        >
          <div className="flex items-start">
            {/* アイコンスケルトン */}
            <div className="w-10 h-10 rounded-full bg-sunken mr-3 flex-shrink-0"></div>
            <div className="flex-1">
              {/* タイトルスケルトン */}
              <div className="h-5 bg-sunken rounded-control mb-2 w-3/4"></div>
              {/* 説明スケルトン */}
              <div className="space-y-2">
                <div className="h-3 bg-sunken rounded-control w-full"></div>
                <div className="h-3 bg-sunken rounded-control w-4/5"></div>
              </div>
            </div>
          </div>

          {/* タグスケルトン */}
          <div className="flex flex-wrap gap-2 mt-4">
            <div className="h-5 bg-sunken rounded-full w-12"></div>
            <div className="h-5 bg-sunken rounded-full w-16"></div>
            <div className="h-5 bg-sunken rounded-full w-14"></div>
          </div>

          {/* 作者スケルトン */}
          <div className="mt-4">
            <div className="h-3 bg-sunken rounded-control w-20"></div>
          </div>
        </div>
      ))}
    </div>
  )
}
