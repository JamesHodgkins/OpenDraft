#ifndef OD_GR_UI_TABVIEW_CPP
#define OD_GR_UI_TABVIEW_CPP

/**************************************************************************************
* OpenDraft:    GUI TabView Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdTabView.cpp                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 22, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI tab view class derived from OdComponent                                     *
***************************************************************************************/



#include "Graphics/UI/OdTabView.h"


namespace OD
{
	namespace Graphics
	{
		//
		// Tab Component for Tab View
		//
		OdTab::OdTab(std::string aText)
		{
			// Create associated button
			button = new OdButton(0, 0, 10, 30, aText);
			addChildControl(button);

			setText(aText);
		}

		OdTab::~OdTab()
		{
			// Delete button
			delete button;
		}

		void OdTab::setText(std::string aText)
		{
			text = aText;
			button->setText(aText);
			resizeFlag = true; // Size needs to be recalculated
		}

		void OdTab::recalculateSize(NVGcontext* aContext)
		{
			// Use nanovg to calculate text width
			float bounds[4];

			nvgReset(aContext);
			nvgTextBounds(aContext, button->location.x, button->location.y, button->text.c_str(), nullptr, bounds);

			int padding = ((OdTabView*)parent)->PADDING;
			int newSize = static_cast<int>(bounds[2] - bounds[0]) + padding; // Where bounds[2]-[0] is the width of the text
			button->size.x = newSize;  

			// Reset flag
			resizeFlag = false;
		}

		void OdTab::onFrame(NVGcontext* aContext)
		{
			// If button's size needs to be recalculated, do so
			if (resizeFlag)
				recalculateSize(aContext);

			// Draw tab button
			button->onFrame(aContext);
		}

		void OdTab::setParent(OdComponent* aParent)
		{
			parent = aParent;

			// Inherit properties from parent OdTabView
			button->size.y = ((OdTabView*)parent)->HEADER_HEIGHT - 4;
			button->setFontSize( ((OdTabView*)parent)->HEADER_FONT_SIZE );
		}



		OdTabView::OdTabView(int aX, int aY, int aWidth, int aHeight)
		{
			location.x = aX;
			location.y = aY;
			size.x = aWidth;
			size.y = aHeight;

			backColour = OdColour::BACKGROUND1;
			headerColour = OdColour::BACKGROUND2;
			foreColour = OdColour::WHITE;

			stroke = OdColour(0, 0, 0, 0);

			text = "TabView";
		}



		//
		// Tab View Component
		//
		OdTabView::~OdTabView()
		{
			// Delete tabs
			for (int i = 0; i < childComponents.size(); i++)
			{
				removeTab(i);
			}
		}


		void OdTabView::onFrame(NVGcontext* aContext)
		{
			if (!enabled)
				return;

			if (aContext == nullptr)
				return;

			int x = getRelativeLocation().x;
			int y = getRelativeLocation().y;

			// Drawing window
			OdDraw::Rect(aContext, x, y, size.x, size.y, OdColour::BACKGROUND2);
			
			OdDraw::RectStroke(aContext, x, y, size.x, size.y, stroke);

			// Draw header
			OdDraw::Rect(aContext, x, y, size.x, HEADER_HEIGHT, OdColour::BACKGROUND1);
			OdDraw::RectStroke(aContext, x, y, size.x, HEADER_HEIGHT, stroke);

			// Create running offset for tab buttons
			int offset = 0;


			//
			// Draw Tabs
			//
			for (int i = 0; i < childComponents.size(); i++)
			{
				//
				// Draw Buttons
				//
				 
				// Update offset
				OdTab* tab = (OdTab*)childComponents[i];
				tab->button->setLocation(offset + 4, 3);
				
				// Draw tab
				tab->onFrame(aContext);

				// Update offset
				offset += tab->button->getSize().x;

				//
				// Draw Tab Content
				//

			}
		}

		int OdTabView::getIndexOfTab(OdTab* aTab)
		{
			// Find index of tab
			for (int i = 0; i < childComponents.size(); i++)
			{
				if (childComponents[i] == aTab)
					return i;
			}
		}

		void OdTabView::addTab(std::string aText)
		{
			// Add tab and associated tab button
			OdTab* tab = new OdTab(aText);

			// Set tab's parent
			tab->setParent(this);

			// Insert into vector
			childComponents.push_back(tab);
		}

		
		void OdTabView::removeTab(int aIndex)
		{
			// Remove tab and associated tab button by index
			delete childComponents[aIndex];

			// Remove from vector
			childComponents.erase(childComponents.begin() + aIndex);
		}
		

	} // namespace Graphics
} // namespace OpenDraft

#endif // OD_GR_UI_BUTTON_CPP