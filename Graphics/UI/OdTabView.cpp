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

			// Create associated panel
			panel = new OdPanel(0, 0, 10, 10);
			addChildControl(panel);

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

			// Static cast properties
			float buttonX = button->getWidth();
			float buttonY = button->getHeight();

			nvgReset(aContext);
			nvgTextBounds(aContext, buttonX, buttonY, button->text.c_str(), nullptr, bounds);

			int padding = ((OdTabView*)parent)->PADDING;
			int newSize = (bounds[2] - bounds[0]) + padding; // Where bounds[2]-[0] is the width of the text
			button->setWidth(newSize);

			// Reset flag
			resizeFlag = false;
		}


		void OdTab::addPanelChildControl(OdComponent* aChild)
		{	
			// Add component to list
			panel->addChildControl(aChild);
		}


		void OdTab::onFrame(NVGcontext* aContext)
		{
			// If button's size needs to be recalculated, do so
			if (resizeFlag)
				recalculateSize(aContext);

			// Draw tab button
			button->onFrame(aContext);

			// Draw tab panel is tab is active (enabled)
			if (enabled)
				panel->onFrame(aContext);
		}

		void OdTab::setParent(OdComponent* aParent)
		{
			parent = aParent;

			// Inherit properties from parent OdTabView
			button->setHeight( ((OdTabView*)parent)->HEADER_HEIGHT - 4);
			button->setFontSize( ((OdTabView*)parent)->HEADER_FONT_SIZE );

			// Set panel location
			panel->setLocation(0, ((OdTabView*)parent)->HEADER_HEIGHT);
			panel->setSize(parent->getWidth(), parent->getHeight() - ((OdTabView*)parent)->HEADER_HEIGHT);
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

			
			// Update states
			for (OdComponent* component : childComponents)
			{
				if (OdTab* tab = dynamic_cast<OdTab*>(component))
				{
					if (tab->button->isMouseDown())
						setActiveTab(getIndexOfTab(tab));
				}
			}


			// Static cast properties
			float x = getRelativeLocation().x;
			float y = getRelativeLocation().y;
			float w = getWidth();
			float h = getHeight();

			// Drawing window
			OdDraw::Rect(aContext, x, y, w, h, OdColour::BACKGROUND2);
			
			OdDraw::RectStroke(aContext, x, y, w, h, stroke);

			// Draw header
			OdDraw::Rect(aContext, x, y, w, HEADER_HEIGHT, OdColour::BACKGROUND1);
			OdDraw::RectStroke(aContext, x, y, h, HEADER_HEIGHT, stroke);

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
				offset += static_cast<int>(tab->button->getWidth());

				//
				// Draw Tab Content
				//

			}
		}

		OdTab* OdTabView::getActiveTab()
		{
			// Find and return active tab
			for (int i = 0; i < childComponents.size(); i++)
			{
				if (childComponents[i]->enabled)
					return (OdTab*)childComponents[i];
			}
		}
		
		OdTab* OdTabView::getTab(int aIndex)
		{
			// Check index is valid
			if (aIndex < 0 || aIndex >= childComponents.size())
				return nullptr;

			// Get tab by index
			return (OdTab*)childComponents[aIndex];
		}

		int OdTabView::getIndexOfTab(OdTab* aTab)
		{
			// Find index of tab
			for (int i = 0; i < childComponents.size(); i++)
			{
				if (childComponents[i] == aTab)
					return i;
			}
			return -1;
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

		void OdTabView::setActiveTab(int aIndex)
		{
			// Check index is valid
			if (aIndex < 0 || aIndex >= childComponents.size())
			{
				// By default, set to first tab
				activeTab = 0;

				// Check not empty
				if (childComponents.size() != 0)
					childComponents[0]->enabled = true;
				
				return;
			}

			// Set tab to active
			for (int i = 0; i < childComponents.size(); i++)
			{
				if (i == aIndex)
					childComponents[i]->enabled = true;
				else
					childComponents[i]->enabled = false;
			}
		}
		

	} // namespace Graphics
} // namespace OpenDraft

#endif // OD_GR_UI_BUTTON_CPP