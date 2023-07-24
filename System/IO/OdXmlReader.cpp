#ifndef OD_IO_XMLREADER_CPP
#define OD_IO_XMLREADER_CPP

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdXmlReader.cpp                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         July 01, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A XML file IO class.                                                              *
***************************************************************************************/


#include "System/IO/OdXmlReader.h"


using namespace pugi;


namespace OD::IO
{
	// Constructor
	OdXmlReader::OdXmlReader(const char* aFilepath) : filepath(aFilepath)
	{
		// Check if the file exists
		if (std::filesystem::exists(aFilepath))
			doc.load_file(aFilepath);
	}

	// Destructor
	OdXmlReader::~OdXmlReader()
	{

	}

	// Save the file
	void OdXmlReader::save()
	{
		// Save the file
		doc.save_file(filepath);
	}

}// namespace OD::IO

#endif // !OD_IO_XMLREADER_CPP
