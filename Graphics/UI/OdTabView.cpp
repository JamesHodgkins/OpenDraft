#ifndef OD_GR_UI_TABVIEW_CPP
#define OD_GR_UI_TABVIEW_CPP

/**************************************************************************************
* OpenDraft:    GUI TabView Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdTabView.cpp                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 22, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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

			nvgSave(aContext);

			nvgReset(aContext);
			nvgTextBounds(aContext, buttonX, buttonY, button->text.c_str(), nullptr, bounds);

			int padding = ((OdTabView*)parent)->PADDING;
			int newSize = (bounds[2] - bounds[0]) + padding; // Where bounds[2]-[0] is the width of the text
			button->setWidth(newSize);

			nvgRestore(aContext);

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

			panel->setWidth(parent->getWidth());

			// Draw tab panel is tab is active (enabled)
			if (enabled)
				panel->onFrame(aContext);
		}


		void OdTab::actionEvents(GrInputMap* aInput)
		{

		}


		void OdTab::setParent(OdComponent* aParent)
		{
			parent = aParent;

			// Inherit properties from parent OdTabView
			button->setHeight( ((OdTabView*)parent)->HEADER_HEIGHT);
			button->setFontSize( ((OdTabView*)parent)->HEADER_FONT_SIZE );

			// Set panel location
			panel->setLocation(0, ((OdTabView*)parent)->HEADER_HEIGHT);
			panel->setSize(parent->getWidth(), parent->getHeight() - ((OdTabView*)parent)->HEADER_HEIGHT);
			panel->backColour = OdColour::BACKGROUND2;
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
			// Update states
			for (std::shared_ptr<OdComponent> component : childComponents)
			{
				if (OdTab* tab = static_cast<OdTab*>(component.get()))
				{
					if (tab->button->wasMousePressed())
						setActiveTab(getIndexOfTab(tab));
				}
			}


			// Static cast properties
			float x = getLocation().x;
			float y = getLocation().y;
			float w = getWidth();
			float h = getHeight();



			// Drawing window
			//OdDraw::Rect(aContext, x, y, w, h, OdColour::PRIMARY);
			
			OdDraw::RectStroke(aContext, x, y, w, h, 0.5, stroke);

			// Draw header
			OdDraw::Rect(aContext, x, y, w, HEADER_HEIGHT, OdColour::BACKGROUND1);
			OdDraw::RectStroke(aContext, x, y, h, HEADER_HEIGHT, 0.5, stroke);

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
				OdTab* tab = (OdTab*)childComponents[i].get();
				tab->button->setLocation(offset + 4, 0);
				
				// Translate by location
				OdDraw::Translate(aContext, x, y);
				
				// Draw tab
				tab->onFrame(aContext);

				// Translate back
				OdDraw::Translate(aContext, -x, -y);
				
				// Update offset
				offset += static_cast<int>(tab->button->getWidth());

			}
		}


		void OdTabView::actionEvents(GrInputMap* aInput)
		{

		}


		OdTab* OdTabView::getActiveTab()
		{
			// Find and return active tab
			for (int i = 0; i < childComponents.size(); i++)
			{
				if (childComponents[i]->enabled)
					return (OdTab*)childComponents[i].get();
			}
			return nullptr;
		}
		
		OdTab* OdTabView::getTab(int aIndex)
		{
			// Check index is valid
			if (aIndex < 0 || aIndex >= childComponents.size())
				return nullptr;

			// Get tab by index
			return (OdTab*)childComponents[aIndex].get();
		}

		int OdTabView::getIndexOfTab(OdTab* aTab)
		{
			// Find index of tab
			for (int i = 0; i < childComponents.size(); i++)
			{
				if (childComponents[i].get() == aTab)
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
			childComponents.push_back(std::shared_ptr<OdComponent>(tab));
		}

		
		void OdTabView::removeTab(int aIndex)
		{
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

			activeTab = aIndex;

			// Set tab to active
			for (int i = 0; i < childComponents.size(); i++)
			{
				// Get child as OdTab
				OdTab* tab = static_cast<OdTab*>(childComponents[i].get());

				if (i == aIndex)
				{
					tab->enabled = true;
					tab->button->backColour = OdColour::BACKGROUND2;
					tab->button->foreColour = OdColour::PRIMARY;
				}
				else
				{
					tab->enabled = false;
					tab->button->backColour = OdColour::BACKGROUND1;
					tab->button->foreColour = OdColour::WHITE;
				}
			}
		}
		

	} // namespace Graphics
} // namespace OpenDraft

#endif // !OD_GR_UI_BUTTON_CPP