#ifndef OD_SY_IMAGE_H
#define OD_SY_IMAGE_H

/**************************************************************************************
* OpenDraft:    System Image Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdImage.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 15, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   An image wrapper class for the OpenDraft framework                                 *
***************************************************************************************/


namespace OD::System
{
	// Image class
	class OdImage {
	private:
		int width, height, id;
		double alpha;
		const char* filePath;

	public:
		OdImage(int aWidth, int aHeight, float aAlpha, const char* aFilePath, int aId)
			: width(aWidth), height(aHeight), alpha(static_cast<double>(aAlpha)), filePath(aFilePath), id(aId) {}

		// Getters
		int getWidth() const { return width; }
		int getHeight() const { return height; }
		double getAlpha() const { return alpha; }
		const char* getFilePath() const { return filePath; }
		int getId() const { return id; }
	};

} // namespace OD::System

#endif // OD_SY_IMAGE_H