#ifndef OD_GR_UI_LABEL_CPP
#define OD_GR_UI_LABEL_CPP

/**************************************************************************************
* OpenDraft:    GUI Label Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdLabel.cpp                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 23, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI label class derived by OdComponent                                          *
***************************************************************************************/



#include <iostream>
#include <sstream>
#include "Graphics/UI/OdLabel.h"


namespace OD
{
	namespace Graphics
	{

			/**
			 * \brief Constructs a Label object with the specified attributes.
			 * \param location (OdPoint) The position of the Label.
			 * \param size (OdPoint) The size of the Label.
			 * \param text (std::string) The text displayed on the Label.
			 */
			OdLabel::OdLabel(int aX, int aY, int aWidth, int aHeight, std::string aText)
			{
				setLocation(aX, aY);
				setSize(aWidth, aHeight);

				backColour = OdColour::CLEAR;
				foreColour = OdColour(255, 255, 255, 200);

				text = aText;
			}


			// Getters and Setters

			// Getters and Setters
			bool OdLabel::isSingleLine() const
			{
				return singleLine;
			}

			void OdLabel::setSingleLine(bool aSingleLine)
			{
				singleLine = aSingleLine;
			}

			bool OdLabel::isTextWrap() const
			{
				return wrapText;
			}

			void OdLabel::setTextWrap(bool aWrapText)
			{
				wrapText = aWrapText;
			}


			/**
			 * \brief Renders a default OD-GUI Label to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			void OdLabel::onFrame(NVGcontext* aContext)
			{
				if (aContext == nullptr)
					return;

				//
				// Begin drawing Label
				//
				float x = getLocation().x;
				float y = getLocation().y;
				float w = size.x;
				float h = size.y;

				OdDraw::TextStyle labelTextStyle =
				{
					14,
					"sans",
					foreColour,
					OdAlign(OdAlign::LEFT | OdAlign::MIDDLE)
				};

				if (singleLine)
				{
					// Get bounding box
					float x = location.x;
					float y = location.y;
					float w = size.x;
					float h = size.y;

					nvgSave(aContext);
					nvgScissor(aContext, x, y, w, h);

					OdDraw::Rect(aContext, x, y, w, h, backColour);
					OdDraw::Text(aContext, x, y, w, h, &labelTextStyle, text.c_str());

					nvgRestore(aContext);
				}
				else if (!wrapText)
				{
					// Get bounding box
					OdDraw::Rect(aContext, x, y, w, h, backColour);

					// Get bounding box
					std::vector<std::string> lines;
					std::istringstream iss(text);
					std::string line;

					// Split text into lines
					while (std::getline(iss, line, '\n')) {
						lines.push_back(line);
					}

					// Draw each line
					for (int i = 0; i < lines.size(); i++)
					{
						float lineY = y + (i * lineSpacingFactor * labelTextStyle.size);
						OdDraw::Text(aContext, x, lineY, w, h, &labelTextStyle, lines[i].c_str());
					}



				}




			}

			void OdLabel::actionEvents(GrInputMap* aInput)
			{

			}


	} // namespace Graphics
} // namespace OD

#endif // !OD_GR_UI_LABEL_CPP