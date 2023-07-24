#ifndef OD_IO_XMLREADER_H
#define OD_IO_XMLREADER_H

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdXmlReader.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         July 01, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A XML file IO class.                                                              *
***************************************************************************************/


#include <unordered_map>						// Include unordered_map
#include <filesystem>							// Include filesystem
#include <iostream>								// Include iostream
#include <Libraries/pugixml/src/pugixml.hpp>	// Include pugixml


namespace OD
{
	namespace IO
	{
		// Application Class
		class OdXmlReader
		{
		private:
			
			// XML document
			pugi::xml_document doc;

			// Filepath
			const char* filepath;

		public:

			// Constructor
			OdXmlReader(const char* aFilepath);

			// Destructor
			~OdXmlReader();

			// Save the file
			void save();

		};

	}// namespace OD::IO
} // namespace OD

#endif // !OD_IO_XMLREADER_H
