#ifndef OD_GR_UI_TABVIEW_H
#define OD_GR_UI_TABVIEW_H

/**************************************************************************************
* OpenDraft:    GUI TabView Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdTabView.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 22, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI tab view class derived from OdComponent                                     *
***************************************************************************************/



#include <string>
#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"
#include "Graphics/UI/OdButton.h"
#include "Graphics/UI/OdPanel.h"
#include "Graphics/UI/OdInput.h"


namespace OD
{
	namespace Graphics
	{
		class OdTab : public OdComponent
		{
		protected:
			bool resizeFlag = true; // Flag to recalculate size on text change

		public:
			OdButton* button;
			OdPanel* panel;

			OdTab(std::string aText);
			~OdTab();

			void setText(std::string aText);
			void recalculateSize(NVGcontext* aContext);
			void addPanelChildControl(OdComponent* aControl);
			void onFrame(NVGcontext* aContext);
			void actionEvents(GrInputMap* aInput);
			void setParent(OdComponent* aParent);

		};



		class OdTabView : public OdComponent
		{
		protected:
			// Style constants
			const int HEADER_HEIGHT = 26;
			const int HEADER_FONT_SIZE = 12;
			const int PADDING = 4;

			int activeTab = 0;

		public:

			OdColour headerColour;

			OdTabView(int aX = 0, int aY = 0, int aWidth = 400, int aHeight = 600);
			~OdTabView();

			void onFrame(NVGcontext* aContext) override;
			void actionEvents(GrInputMap* aInput) override;

			// Tab management
			OdTab* getActiveTab();
			OdTab* getTab(int aIndex);
			int getIndexOfTab(OdTab* aTab);
			void addTab(std::string aText);
			void removeTab(int aIndex);
			void setActiveTab(int aIndex);

			// Set OdTab class as a friend so it can access protected members
			friend class OdTab;
		};

	} // namespace Graphics
} // namespace OD

#endif // !OD_GR_UI_TABVIEW_H
